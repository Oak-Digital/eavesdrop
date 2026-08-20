use std::{
    io::Cursor,
    path::{Path, PathBuf},
};

use chrono::Utc;
use symphonia::core::{
    audio::{Channels, SampleBuffer},
    codecs::{CODEC_TYPE_AAC, CodecParameters, DecoderOptions},
    errors::Error as SymphoniaError,
    formats::Packet,
};
use tauri::{AppHandle, Emitter};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, get_lang_str_full,
};

use crate::{
    download::{self, Checksum},
    error::{AppError, AppResult},
    models::{
        Transcript, TranscriptSegment, TranscriptionProgress, TranscriptionStage,
        WhisperModelDownloadProgress, WhisperModelInfo,
    },
};

const WHISPER_SAMPLE_RATE: u32 = 16_000;
const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

struct ModelSpec {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    size_bytes: u64,
    sha1: &'static str,
}

const MODELS: &[ModelSpec] = &[
    ModelSpec {
        id: "tiny",
        name: "Tiny",
        description: "Fastest, with lower accuracy",
        size_bytes: 77_691_713,
        sha1: "bd577a113a864445d4c299885e0cb97d4ba92b5f",
    },
    ModelSpec {
        id: "base",
        name: "Base",
        description: "Recommended balance of speed and accuracy",
        size_bytes: 147_951_465,
        sha1: "465707469ff3a37a2b9b8d8f89f2f99de7299dac",
    },
    ModelSpec {
        id: "small",
        name: "Small",
        description: "More accurate, but slower",
        size_bytes: 487_601_967,
        sha1: "55356645c2b361a969dfd0ef2c5a50d530afd8d5",
    },
];

pub fn available_models(models_dir: &Path) -> Vec<WhisperModelInfo> {
    MODELS
        .iter()
        .map(|model| WhisperModelInfo {
            id: model.id.into(),
            name: model.name.into(),
            description: model.description.into(),
            size_bytes: model.size_bytes,
            installed: model_path(models_dir, model).is_file(),
        })
        .collect()
}

pub fn install_model(app: &AppHandle, models_dir: &Path, model_id: &str) -> AppResult<PathBuf> {
    let model = MODELS
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| AppError::State("unknown Whisper model".into()))?;
    download::install(
        &format!("{MODEL_BASE_URL}/ggml-{}.bin", model.id),
        &model_path(models_dir, model),
        model.size_bytes,
        Checksum::Sha1(model.sha1),
        "Whisper model",
        |downloaded, total| emit_download_progress(app, model, downloaded, total),
    )
}

fn emit_transcription_progress(
    app: &AppHandle,
    recording_id: &str,
    stage: TranscriptionStage,
    progress: f32,
) {
    let _ = app.emit(
        "transcription-progress",
        TranscriptionProgress {
            recording_id: recording_id.to_string(),
            stage,
            progress: progress.clamp(0.0, 1.0),
        },
    );
}

fn emit_download_progress(app: &AppHandle, model: &ModelSpec, downloaded: u64, total: u64) {
    let _ = app.emit(
        "whisper-model-download-progress",
        WhisperModelDownloadProgress {
            model_id: model.id.into(),
            downloaded_bytes: downloaded,
            total_bytes: total,
        },
    );
}

fn model_path(models_dir: &Path, model: &ModelSpec) -> PathBuf {
    models_dir.join(format!("ggml-{}.bin", model.id))
}

pub fn transcribe(
    app: &AppHandle,
    recording_id: &str,
    m4a: &[u8],
    model_path: &Path,
) -> AppResult<Transcript> {
    if !model_path.is_file() {
        return Err(AppError::State(format!(
            "Whisper model was not found at {}",
            model_path.display()
        )));
    }

    emit_transcription_progress(app, recording_id, TranscriptionStage::Decoding, 0.0);
    let pcm = decode_m4a_to_mono(m4a)?;
    if pcm.is_empty() {
        return Err(AppError::Audio(
            "recording contains no audio to transcribe".into(),
        ));
    }

    let context = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
        .map_err(|error| AppError::Other(format!("could not load Whisper model: {error}")))?;
    let mut state = context
        .create_state()
        .map_err(|error| AppError::Other(format!("could not initialize Whisper: {error}")))?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 2 });
    let threads = std::thread::available_parallelism()
        .map(|count| count.get().saturating_sub(1).clamp(1, 8))
        .unwrap_or(2);
    params.set_n_threads(threads as i32);
    params.set_translate(false);
    params.set_language(None);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    // Whisper reports 0..=100. Emit only on change: the callback fires far more
    // often than the bar can meaningfully move, and every emit crosses to the
    // webview.
    emit_transcription_progress(app, recording_id, TranscriptionStage::Transcribing, 0.0);
    let progress_app = app.clone();
    let progress_id = recording_id.to_string();
    let mut last_percent = -1i32;
    params.set_progress_callback_safe(move |percent: i32| {
        let percent = percent.clamp(0, 100);
        if percent == last_percent {
            return;
        }
        last_percent = percent;
        emit_transcription_progress(
            &progress_app,
            &progress_id,
            TranscriptionStage::Transcribing,
            percent as f32 / 100.0,
        );
    });

    state
        .full(params, &pcm)
        .map_err(|error| AppError::Other(format!("Whisper transcription failed: {error}")))?;
    // Whisper's callback is not guaranteed to land on exactly 100, so settle the
    // bar at full before the caller tears the UI down.
    emit_transcription_progress(app, recording_id, TranscriptionStage::Transcribing, 1.0);

    let mut segments = Vec::new();
    for segment in state.as_iter() {
        let text = segment
            .to_str_lossy()
            .map_err(|error| AppError::Other(format!("Whisper returned invalid text: {error}")))?
            .trim()
            .to_string();
        if !text.is_empty() {
            segments.push(TranscriptSegment {
                start_ms: segment.start_timestamp().saturating_mul(10),
                end_ms: segment.end_timestamp().saturating_mul(10),
                text,
            });
        }
    }
    let text = segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let language = get_lang_str_full(state.full_lang_id_from_state()).map(str::to_string);

    Ok(Transcript {
        text,
        language,
        created_at: Utc::now().to_rfc3339(),
        segments,
    })
}

fn decode_m4a_to_mono(m4a: &[u8]) -> AppResult<Vec<f32>> {
    let mut reader = mp4::Mp4Reader::read_header(Cursor::new(m4a), m4a.len() as u64)
        .map_err(|error| AppError::Audio(format!("could not open recording: {error}")))?;
    let (track_id, source_rate, channels, sample_count) = {
        let (track_id, track) = reader
            .tracks()
            .iter()
            .find(|(_, track)| matches!(track.media_type(), Ok(mp4::MediaType::AAC)))
            .ok_or_else(|| AppError::Audio("recording has no AAC audio track".into()))?;
        let source_rate = track
            .sample_freq_index()
            .map_err(|error| AppError::Audio(format!("invalid recording sample rate: {error}")))?
            .freq();
        let channels = match track.channel_config().map_err(|error| {
            AppError::Audio(format!("invalid recording channel layout: {error}"))
        })? {
            mp4::ChannelConfig::Mono => Channels::FRONT_LEFT,
            mp4::ChannelConfig::Stereo => Channels::FRONT_LEFT | Channels::FRONT_RIGHT,
            _ => {
                return Err(AppError::Audio(
                    "recordings with more than two channels are unsupported".into(),
                ));
            }
        };
        (*track_id, source_rate, channels, track.sample_count())
    };
    let mut codec_params = CodecParameters::new();
    codec_params
        .for_codec(CODEC_TYPE_AAC)
        .with_sample_rate(source_rate)
        .with_channels(channels);
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|error| AppError::Audio(format!("could not initialize AAC decoder: {error}")))?;
    let mut mono = Vec::new();

    for sample_id in 1..=sample_count {
        let sample = reader
            .read_sample(track_id, sample_id)
            .map_err(|error| AppError::Audio(format!("could not read recording audio: {error}")))?
            .ok_or_else(|| AppError::Audio("recording audio sample is missing".into()))?;
        let packet = Packet::new_from_slice(
            track_id,
            sample.start_time,
            sample.duration as u64,
            &sample.bytes,
        );
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(error) => {
                return Err(AppError::Audio(format!(
                    "could not decode recording audio: {error}"
                )));
            }
        };
        let channels = decoded.spec().channels.count();
        if channels == 0 {
            continue;
        }
        let mut interleaved = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        interleaved.copy_interleaved_ref(decoded);
        for frame in interleaved.samples().chunks_exact(channels) {
            mono.push(frame.iter().copied().sum::<f32>() / channels as f32);
        }
    }

    Ok(resample(&mono, source_rate, WHISPER_SAMPLE_RATE))
}

fn resample(input: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if input.is_empty() || source_rate == 0 || target_rate == 0 {
        return Vec::new();
    }
    if source_rate == target_rate {
        return input.to_vec();
    }
    let output_len = ((input.len() as u64 * target_rate as u64) / source_rate as u64) as usize;
    let step = source_rate as f64 / target_rate as f64;
    (0..output_len)
        .map(|index| {
            let position = index as f64 * step;
            let left = position.floor() as usize;
            let right = (left + 1).min(input.len() - 1);
            let fraction = (position - left as f64) as f32;
            input[left] + (input[right] - input[left]) * fraction
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_aac::{AacEncoder, AacEncoderConfig};

    #[test]
    fn resamples_recording_audio_to_whisper_rate() {
        let source = vec![0.25; 48_000];
        let output = resample(&source, 48_000, WHISPER_SAMPLE_RATE);
        assert_eq!(output.len(), 16_000);
        assert!(
            output
                .iter()
                .all(|sample| (*sample - 0.25).abs() < f32::EPSILON)
        );
    }

    #[test]
    fn decodes_eavesdrop_m4a_for_whisper() {
        let samples: Vec<f32> = (0..48_000)
            .map(|index| (index as f32 * 220.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.1)
            .collect();
        let mut encoder = AacEncoder::new(AacEncoderConfig {
            bitrate_bps: 96_000,
            ..Default::default()
        });
        encoder.push_pcm(&samples, 1, 48_000).unwrap();
        encoder.finish();
        let mut packets = Vec::new();
        while let Ok(packet) = encoder.next_packet() {
            packets.push(packet.data);
        }
        let m4a = crate::audio::build_m4a(&packets).unwrap();

        let decoded = decode_m4a_to_mono(&m4a).unwrap();

        assert!((15_000..=17_000).contains(&decoded.len()));
        assert!(decoded.iter().any(|sample| sample.abs() > 0.01));
    }

    #[test]
    fn model_catalog_detects_downloaded_models() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("ggml-base.bin"), b"model").unwrap();

        let models = available_models(temp.path());

        assert_eq!(models.len(), 3);
        assert!(
            models
                .iter()
                .find(|model| model.id == "base")
                .unwrap()
                .installed
        );
        assert!(
            !models
                .iter()
                .find(|model| model.id == "tiny")
                .unwrap()
                .installed
        );
    }

    #[test]
    fn model_catalog_publishes_a_sha1_for_every_entry() {
        assert!(MODELS.iter().all(|model| model.sha1.len() == 40));
    }
}
