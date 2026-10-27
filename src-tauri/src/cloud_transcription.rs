//! Explicit cloud transcription. Audio is decrypted and split in memory only.
//! API contract: https://developers.openai.com/api/docs/guides/speech-to-text
use std::{io::Read, time::Duration};

use chrono::Utc;
use reqwest::blocking::{Client, multipart};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;

use crate::{
    cloud_summarization::{CloudProvider, load_key},
    error::{AppError, AppResult},
    models::{Transcript, TranscriptionStage},
    transcription::{decode_m4a_to_mono, emit_transcription_progress},
};

pub const MODEL: &str = "gpt-transcribe";
const ENDPOINT: &str = "https://api.openai.com/v1/audio/transcriptions";
// Leave room below the provider's 25 MB limit for multipart framing.
const MAX_UPLOAD_BYTES: usize = 24_000_000;
const SAMPLE_RATE: usize = 16_000;
const MAX_CHUNK_SAMPLES: usize = SAMPLE_RATE * 60 * 10;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptionSource {
    #[default]
    Local,
    Cloud,
}
impl TranscriptionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Cloud => "cloud",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TranscriptionTarget {
    pub source: TranscriptionSource,
}

pub fn transcribe(app: &AppHandle, id: &str, audio: &[u8]) -> AppResult<Transcript> {
    if audio.is_empty() {
        return Err(AppError::Audio(
            "recording contains no audio to transcribe".into(),
        ));
    }
    let key = load_key(CloudProvider::Openai)?;
    if audio.len() <= MAX_UPLOAD_BYTES {
        emit_transcription_progress(app, id, TranscriptionStage::CloudTranscribing, 0.0);
        let result = send_request(ENDPOINT, &key, audio.to_vec(), "recording.m4a", "audio/mp4")?;
        let transcript = combine_responses(vec![result])?;
        emit_transcription_progress(app, id, TranscriptionStage::CloudTranscribing, 1.0);
        return Ok(transcript);
    }

    emit_transcription_progress(app, id, TranscriptionStage::Decoding, 0.0);
    let pcm = decode_m4a_to_mono(audio)?;
    transcribe_chunks(
        &pcm,
        |chunk| {
            send_request(
                ENDPOINT,
                &key,
                wav_bytes(chunk)?,
                "recording.wav",
                "audio/wav",
            )
        },
        |progress| {
            emit_transcription_progress(app, id, TranscriptionStage::CloudTranscribing, progress)
        },
    )
}

fn transcribe_chunks(
    pcm: &[f32],
    mut request: impl FnMut(&[f32]) -> AppResult<Value>,
    mut progress: impl FnMut(f32),
) -> AppResult<Transcript> {
    if pcm.is_empty() {
        return Err(AppError::Audio(
            "recording contains no audio to transcribe".into(),
        ));
    }
    let mut responses = Vec::new();
    let mut start = 0;
    progress(0.0);
    while start < pcm.len() {
        let end = start + chunk_length(&pcm[start..]);
        let response = request(&pcm[start..end])?;
        response_text(&response)?;
        responses.push(response);
        start = end;
        progress(start as f32 / pcm.len() as f32);
    }
    combine_responses(responses)
}

fn chunk_length(pcm: &[f32]) -> usize {
    if pcm.len() <= MAX_CHUNK_SAMPLES {
        return pcm.len();
    }
    // Prefer a quiet half-second in the last 30 seconds to avoid mid-word cuts.
    let window = SAMPLE_RATE / 2;
    let search_start = MAX_CHUNK_SAMPLES - SAMPLE_RATE * 30;
    let quiet = pcm[search_start..MAX_CHUNK_SAMPLES]
        .chunks_exact(window)
        .enumerate()
        .map(|(index, samples)| {
            (
                index,
                samples.iter().map(|v| v.abs()).sum::<f32>() / window as f32,
            )
        })
        .filter(|(_, amplitude)| *amplitude < 0.01)
        .min_by(|a, b| a.1.total_cmp(&b.1));
    quiet.map_or(MAX_CHUNK_SAMPLES, |(index, _)| {
        search_start + index * window + window / 2
    })
}

fn wav_bytes(pcm: &[f32]) -> AppResult<Vec<u8>> {
    let data_size = pcm
        .len()
        .checked_mul(2)
        .filter(|size| *size + 44 <= MAX_UPLOAD_BYTES)
        .ok_or_else(|| AppError::Audio("audio chunk exceeds the cloud upload limit".into()))?;
    let mut bytes = Vec::with_capacity(data_size + 44);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(data_size as u32 + 36).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&(SAMPLE_RATE as u32).to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE as u32 * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_size as u32).to_le_bytes());
    for sample in pcm {
        bytes
            .extend_from_slice(&((sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    Ok(bytes)
}

fn send_request(
    endpoint: &str,
    key: &str,
    audio: Vec<u8>,
    filename: &'static str,
    mime: &str,
) -> AppResult<Value> {
    if audio.is_empty() || audio.len() > MAX_UPLOAD_BYTES {
        return Err(AppError::Audio("invalid cloud audio upload size".into()));
    }
    let http = Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(600))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| {
            AppError::Other("could not create the cloud transcription connection".into())
        })?;
    let part = multipart::Part::bytes(audio)
        .file_name(filename)
        .mime_str(mime)
        .map_err(|_| AppError::Other("could not prepare cloud audio".into()))?;
    let form = multipart::Form::new()
        .text("model", MODEL)
        .part("file", part);
    let response = http.post(endpoint).bearer_auth(key).multipart(form).send().map_err(|error| AppError::Other(
        if error.is_timeout() { "cloud transcription timed out; the request may have been processed, so check usage before retrying".into() }
        else { "could not reach OpenAI; check your connection".into() }
    ))?;
    let status = response.status();
    if !status.is_success() {
        // Do not surface response bodies, which could echo audio, text, or keys.
        return Err(AppError::Other(match status.as_u16() {
            401 | 403 => "OpenAI rejected the API key or model access; check your key and account permissions".into(),
            429 => "OpenAI rate limit or quota reached; check usage and billing before retrying".into(),
            400 | 404 | 413 | 422 => "OpenAI rejected the transcription request; check audio format and model access".into(),
            _ => format!("OpenAI transcription failed (HTTP {}); try again later", status.as_u16()),
        }));
    }
    let mut bytes = Vec::new();
    response
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::Other("could not read the cloud transcription response".into()))?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(AppError::Other(
            "cloud transcription response was too large".into(),
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| AppError::Other("OpenAI returned an invalid transcription response".into()))
}

fn response_text(response: &Value) -> AppResult<&str> {
    response
        .get("text")
        .and_then(Value::as_str)
        .map(str::trim)
        .ok_or_else(|| AppError::Other("OpenAI returned an invalid transcription response".into()))
}

fn combine_responses(responses: Vec<Value>) -> AppResult<Transcript> {
    let mut texts = Vec::new();
    let mut languages = Vec::new();
    for response in responses {
        let text = response_text(&response)?;
        if !text.is_empty() {
            texts.push(text.to_owned());
        }
        if let Some(detected) = response.get("languages").and_then(Value::as_array) {
            for language in detected {
                if let Some(code) = language
                    .get("code")
                    .and_then(Value::as_str)
                    .filter(|code| !code.is_empty())
                {
                    if !languages.iter().any(|existing| existing == code) {
                        languages.push(code.to_owned());
                    }
                }
            }
        }
    }
    if texts.is_empty() {
        return Err(AppError::Other(
            "OpenAI returned no speech; the existing transcript has been kept".into(),
        ));
    }
    Ok(Transcript {
        text: texts.join("\n\n"),
        language: (!languages.is_empty()).then(|| languages.join(", ")),
        created_at: Utc::now().to_rfc3339(),
        // gpt-transcribe returns text without timestamps. Never invent segments.
        segments: Vec::new(),
        source: Some(TranscriptionSource::Cloud),
        model: Some(MODEL.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{io::Write, net::TcpListener, thread};

    #[test]
    fn text_only_responses_preserve_languages_without_inventing_timestamps() {
        let transcript = combine_responses(vec![
            json!({"text":" First part. ","languages":[{"code":"da"}]}),
            json!({"text":"Second part.","languages":[{"code":"da"},{"code":"en"}]}),
        ])
        .unwrap();
        assert_eq!(transcript.text, "First part.\n\nSecond part.");
        assert_eq!(transcript.language.as_deref(), Some("da, en"));
        assert!(transcript.segments.is_empty());
        assert_eq!(transcript.source, Some(TranscriptionSource::Cloud));
        assert_eq!(transcript.model.as_deref(), Some(MODEL));
        assert!(
            combine_responses(vec![json!({"text":"Speech","languages":[]})])
                .unwrap()
                .language
                .is_none()
        );
        for invalid in [
            json!({"text":"  "}),
            json!({"text":null}),
            json!({"error":"private provider error"}),
        ] {
            assert!(combine_responses(vec![invalid]).is_err());
        }
        assert!(
            serde_json::from_value::<TranscriptionTarget>(json!({"source":"unknown"})).is_err()
        );
    }

    #[test]
    fn long_audio_is_split_near_silence_without_gaps_and_stays_under_upload_limit() {
        let mut pcm = vec![0.2; MAX_CHUNK_SAMPLES + SAMPLE_RATE];
        let silence_start = MAX_CHUNK_SAMPLES - SAMPLE_RATE * 10;
        pcm[silence_start..silence_start + SAMPLE_RATE / 2].fill(0.0);
        let split = chunk_length(&pcm);
        assert_eq!(split, silence_start + SAMPLE_RATE / 4);
        let mut consumed = 0;
        let mut count = 0;
        let mut progress = Vec::new();
        let transcript = transcribe_chunks(
            &pcm,
            |chunk| {
                assert_eq!(chunk, &pcm[consumed..consumed + chunk.len()]);
                consumed += chunk.len();
                count += 1;
                let wav = wav_bytes(chunk).unwrap();
                assert!(wav.len() < MAX_UPLOAD_BYTES);
                assert_eq!(&wav[..4], b"RIFF");
                assert_eq!(&wav[8..12], b"WAVE");
                assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16000);
                assert_eq!(
                    u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize,
                    chunk.len() * 2
                );
                Ok(json!({"text":format!("Part {count}")}))
            },
            |value| progress.push(value),
        )
        .unwrap();
        assert_eq!(consumed, pcm.len());
        assert_eq!(count, 2);
        assert_eq!(transcript.text, "Part 1\n\nPart 2");
        assert_eq!(progress.first(), Some(&0.0));
        assert_eq!(progress.last(), Some(&1.0));
        assert!(progress.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn a_failed_chunk_returns_no_partial_transcript_and_does_not_retry() {
        let pcm = vec![0.2; MAX_CHUNK_SAMPLES + 1];
        let mut calls = 0;
        let result = transcribe_chunks(
            &pcm,
            |_| {
                calls += 1;
                if calls == 1 {
                    Ok(json!({"text":"First part"}))
                } else {
                    Err(AppError::Other("quota reached".into()))
                }
            },
            |_| {},
        );
        assert!(result.is_err());
        assert_eq!(calls, 2);
        assert!(
            transcribe_chunks(&[], |_| panic!("empty audio must not be uploaded"), |_| {}).is_err()
        );
    }

    fn server(status: u16, response: &str) -> (String, thread::JoinHandle<(String, Vec<u8>)>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let response = response.to_owned();
        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut received = Vec::new();
            let header_end = loop {
                let mut buffer = [0; 1024];
                let count = socket.read(&mut buffer).unwrap();
                assert!(count > 0);
                received.extend_from_slice(&buffer[..count]);
                if let Some(end) = received.windows(4).position(|part| part == b"\r\n\r\n") {
                    break end + 4;
                }
            };
            let headers = String::from_utf8(received[..header_end].to_vec())
                .unwrap()
                .to_lowercase();
            let length: usize = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            while received.len() < header_end + length {
                let mut buffer = [0; 1024];
                let count = socket.read(&mut buffer).unwrap();
                assert!(count > 0);
                received.extend_from_slice(&buffer[..count]);
            }
            write!(socket, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap();
            (headers, received[header_end..header_end + length].to_vec())
        });
        (endpoint, handle)
    }

    #[test]
    fn multipart_upload_uses_the_audio_endpoint_contract_and_sanitizes_errors() {
        let (endpoint, server) = server(200, r#"{"text":"Hello","languages":[{"code":"en"}]}"#);
        let result = send_request(
            &endpoint,
            "dummy-key",
            b"AUDIO_BYTES".to_vec(),
            "recording.m4a",
            "audio/mp4",
        )
        .unwrap();
        assert_eq!(result["text"], "Hello");
        let (headers, body) = server.join().unwrap();
        assert!(headers.contains("authorization: bearer dummy-key"));
        assert!(headers.contains("content-type: multipart/form-data; boundary="));
        let body = String::from_utf8(body).unwrap();
        assert!(body.contains("name=\"model\"\r\n\r\ngpt-transcribe"));
        assert!(body.contains("filename=\"recording.m4a\""));
        assert!(body.contains("Content-Type: audio/mp4"));
        assert!(body.contains("AUDIO_BYTES"));
        assert!(!body.contains("timestamp_granularities"));
        for status in [401, 403, 413, 429, 500, 302] {
            let (endpoint, handle) = self::server(status, "SECRET_AUDIO_TRANSCRIPT_AND_KEY");
            let error = send_request(
                &endpoint,
                "dummy-key",
                vec![1, 2],
                "recording.m4a",
                "audio/mp4",
            )
            .unwrap_err()
            .to_string();
            assert!(!error.contains("SECRET"));
            assert!(!error.contains("dummy-key"));
            handle.join().unwrap();
        }
    }
}
