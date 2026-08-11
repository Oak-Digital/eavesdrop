use std::{
    collections::VecDeque,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use bytes::Bytes;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use mp4::{
    AacConfig, AudioObjectType, ChannelConfig, MediaConfig, Mp4Config, Mp4Sample, Mp4Writer,
    SampleFreqIndex, TrackConfig, TrackType,
};
use rusty_aac::{AacEncoder, AacEncoderConfig};
use tauri::{AppHandle, Emitter};

use crate::{
    crypto::Vault,
    error::{AppError, AppResult},
    models::{AudioDevice, AudioLevels, CaptureMode},
    platform,
};

pub const SAMPLE_RATE: u32 = 48_000;
const FRAME_SAMPLES: usize = 480;
const SEGMENT_SAMPLES: usize = (SAMPLE_RATE as usize) * 2;
const METER_EMIT_INTERVAL: Duration = Duration::from_millis(50);
const METER_STALE_AFTER: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Microphone,
    System,
}

pub struct SourceChunk {
    pub source: SourceKind,
    pub samples: Vec<f32>,
    pub level: f32,
}

pub enum CaptureMessage {
    Samples(SourceChunk),
    Pause,
    Resume,
    Warning(String),
    Stop,
}

pub trait CaptureBackend: Send {
    fn start(
        &mut self,
        mode: CaptureMode,
        microphone_id: Option<&str>,
        sender: Sender<CaptureMessage>,
    ) -> AppResult<()>;
    fn stop(&mut self);
}

pub struct CaptureSession {
    backend: Box<dyn CaptureBackend>,
    sender: Sender<CaptureMessage>,
    worker: Option<JoinHandle<AppResult<WorkerResult>>>,
    paused: Arc<AtomicBool>,
    started: Instant,
    paused_since: Option<Instant>,
    paused_total: Duration,
}

pub struct WorkerResult {
    pub m4a: Vec<u8>,
    pub playable_ms: i64,
    pub warning: Option<String>,
}

impl CaptureSession {
    pub fn start(
        app: AppHandle,
        mode: CaptureMode,
        microphone_id: Option<&str>,
        journal_path: PathBuf,
        recording_key: [u8; 32],
    ) -> AppResult<Self> {
        let (sender, receiver) = mpsc::channel();
        let mut backend = platform::capture_backend();
        backend.start(mode, microphone_id, sender.clone())?;
        let paused = Arc::new(AtomicBool::new(false));
        let worker = thread::Builder::new()
            .name("eavesdrop-audio-worker".into())
            .spawn(move || capture_worker(app, mode, receiver, journal_path, recording_key))
            .map_err(|error| AppError::Audio(error.to_string()))?;
        Ok(Self {
            backend,
            sender,
            worker: Some(worker),
            paused,
            started: Instant::now(),
            paused_since: None,
            paused_total: Duration::ZERO,
        })
    }

    pub fn pause(&mut self) -> AppResult<()> {
        if self.paused.swap(true, Ordering::SeqCst) {
            return Err(AppError::State("recording is already paused".into()));
        }
        self.paused_since = Some(Instant::now());
        self.sender
            .send(CaptureMessage::Pause)
            .map_err(|error| AppError::Audio(error.to_string()))
    }

    pub fn resume(&mut self) -> AppResult<()> {
        if !self.paused.swap(false, Ordering::SeqCst) {
            return Err(AppError::State("recording is not paused".into()));
        }
        if let Some(paused_since) = self.paused_since.take() {
            self.paused_total += paused_since.elapsed();
        }
        self.sender
            .send(CaptureMessage::Resume)
            .map_err(|error| AppError::Audio(error.to_string()))
    }

    pub fn elapsed_ms(&self) -> i64 {
        self.started.elapsed().as_millis() as i64
    }

    pub fn playable_ms(&self) -> i64 {
        let current_pause = self
            .paused_since
            .map(|instant| instant.elapsed())
            .unwrap_or_default();
        self.started
            .elapsed()
            .saturating_sub(self.paused_total + current_pause)
            .as_millis() as i64
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn stop(mut self) -> AppResult<WorkerResult> {
        self.backend.stop();
        let _ = self.sender.send(CaptureMessage::Stop);
        self.worker
            .take()
            .ok_or_else(|| AppError::State("capture worker is missing".into()))?
            .join()
            .map_err(|_| AppError::Audio("capture worker stopped unexpectedly".into()))?
    }
}

pub fn input_devices() -> Vec<AudioDevice> {
    let host = cpal::default_host();
    let default_id = host
        .default_input_device()
        .and_then(|device| device.id().ok())
        .map(|id| id.to_string());
    let Ok(devices) = host.input_devices() else {
        return Vec::new();
    };
    devices
        .filter_map(|device| {
            let name = device.to_string();
            let id = device.id().ok()?.to_string();
            Some(AudioDevice {
                is_default: default_id.as_deref() == Some(id.as_str()),
                id,
                name,
            })
        })
        .collect()
}

pub fn start_microphone_stream(
    microphone_id: Option<&str>,
    sender: Sender<CaptureMessage>,
) -> AppResult<cpal::Stream> {
    let host = cpal::default_host();
    let device = if let Some(id) = microphone_id {
        host.input_devices()
            .map_err(|error| AppError::Audio(error.to_string()))?
            .find(|device| {
                device
                    .id()
                    .ok()
                    .is_some_and(|device_id| device_id.to_string() == id)
            })
            .or_else(|| host.default_input_device())
    } else {
        host.default_input_device()
    }
    .ok_or_else(|| AppError::Audio("no microphone is available".into()))?;

    let supported = device
        .default_input_config()
        .map_err(|error| AppError::Audio(error.to_string()))?;
    let sample_rate = supported.sample_rate();
    let channels = supported.channels() as usize;
    let config: cpal::StreamConfig = supported.into();
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => {
            let tx = sender.clone();
            let error_tx = sender.clone();
            device.build_input_stream(
                config,
                move |data: &[f32], _| {
                    send_pcm(&tx, SourceKind::Microphone, data, channels, sample_rate)
                },
                move |error| {
                    let _ = error_tx.send(CaptureMessage::Warning(format!(
                        "Microphone interrupted: {error}"
                    )));
                },
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let tx = sender.clone();
            let error_tx = sender.clone();
            device.build_input_stream(
                config,
                move |data: &[i16], _| {
                    let converted: Vec<f32> = data
                        .iter()
                        .map(|sample| *sample as f32 / i16::MAX as f32)
                        .collect();
                    send_pcm(
                        &tx,
                        SourceKind::Microphone,
                        &converted,
                        channels,
                        sample_rate,
                    );
                },
                move |error| {
                    let _ = error_tx.send(CaptureMessage::Warning(format!(
                        "Microphone interrupted: {error}"
                    )));
                },
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let tx = sender.clone();
            let error_tx = sender.clone();
            device.build_input_stream(
                config,
                move |data: &[u16], _| {
                    let converted: Vec<f32> = data
                        .iter()
                        .map(|sample| (*sample as f32 / u16::MAX as f32) * 2.0 - 1.0)
                        .collect();
                    send_pcm(
                        &tx,
                        SourceKind::Microphone,
                        &converted,
                        channels,
                        sample_rate,
                    );
                },
                move |error| {
                    let _ = error_tx.send(CaptureMessage::Warning(format!(
                        "Microphone interrupted: {error}"
                    )));
                },
                None,
            )
        }
        format => {
            return Err(AppError::Audio(format!(
                "unsupported microphone sample format: {format}"
            )));
        }
    }
    .map_err(|error| AppError::Audio(error.to_string()))?;
    stream
        .play()
        .map_err(|error| AppError::Audio(error.to_string()))?;
    Ok(stream)
}

pub fn send_pcm(
    sender: &Sender<CaptureMessage>,
    source: SourceKind,
    input: &[f32],
    channels: usize,
    source_rate: u32,
) {
    if input.is_empty() || channels == 0 {
        return;
    }
    let mono: Vec<f32> = input
        .chunks(channels)
        .map(|frame| frame.iter().copied().sum::<f32>() / frame.len() as f32)
        .collect();
    let samples = if source_rate == SAMPLE_RATE {
        mono
    } else {
        linear_resample(&mono, source_rate, SAMPLE_RATE)
    };
    let level = meter_level(&samples);
    let _ = sender.send(CaptureMessage::Samples(SourceChunk {
        source,
        samples,
        level,
    }));
}

fn meter_level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let rms =
        (samples.iter().map(|value| value * value).sum::<f32>() / samples.len() as f32).sqrt();
    let peak = samples
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f32, f32::max);
    let amplitude = rms.max(peak * 0.25);
    if amplitude <= 0.001 {
        return 0.0;
    }

    // Audio amplitude is logarithmic. Mapping the useful -60–0 dBFS range to
    // 0–1 keeps normal speech visible without changing the recorded samples.
    let dbfs = 20.0 * amplitude.log10();
    ((dbfs + 60.0) / 60.0).clamp(0.0, 1.0).powf(0.65)
}

fn smooth_meter(current: f32, target: f32) -> f32 {
    let response = if target > current { 0.65 } else { 0.18 };
    let next = current + (target - current) * response;
    if next < 0.005 && target == 0.0 {
        0.0
    } else {
        next
    }
}

fn linear_resample(input: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if input.len() < 2 || source_rate == 0 {
        return input.to_vec();
    }
    let output_len = ((input.len() as u64 * target_rate as u64) / source_rate as u64) as usize;
    let ratio = source_rate as f64 / target_rate as f64;
    (0..output_len)
        .map(|index| {
            let position = index as f64 * ratio;
            let left = position.floor() as usize;
            let fraction = (position - left as f64) as f32;
            let a = input[left.min(input.len() - 1)];
            let b = input[(left + 1).min(input.len() - 1)];
            a + (b - a) * fraction
        })
        .collect()
}

fn capture_worker(
    app: AppHandle,
    mode: CaptureMode,
    receiver: Receiver<CaptureMessage>,
    journal_path: PathBuf,
    key: [u8; 32],
) -> AppResult<WorkerResult> {
    let mut microphone = VecDeque::new();
    let mut system = VecDeque::new();
    let mut segment = Vec::with_capacity(SEGMENT_SAMPLES);
    let mut paused = false;
    let mut mic_level = 0.0;
    let mut system_level = 0.0;
    let mut mic_target = 0.0;
    let mut system_target = 0.0;
    let mut last_mic_sample = None;
    let mut last_system_sample = None;
    let mut last_meter_emit = Instant::now();
    let mut playable_samples: u64 = 0;
    let mut warning = None;
    let mut last_disk_check = Instant::now();
    let mut low_disk_warning_sent = false;

    #[cfg(feature = "aec3")]
    let echo_processor = {
        use webrtc_audio_processing::Processor;
        use webrtc_audio_processing_config::{Config, EchoCanceller};
        let processor = Processor::new(SAMPLE_RATE as i32)
            .map_err(|error| AppError::Audio(format!("AEC3 initialization failed: {error:?}")))?;
        processor.set_config(Config {
            echo_canceller: Some(EchoCanceller::default()),
            ..Default::default()
        });
        processor
    };

    loop {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(CaptureMessage::Samples(chunk)) if !paused => match chunk.source {
                SourceKind::Microphone => {
                    mic_target = chunk.level;
                    last_mic_sample = Some(Instant::now());
                    microphone.extend(chunk.samples);
                }
                SourceKind::System => {
                    system_target = chunk.level;
                    last_system_sample = Some(Instant::now());
                    system.extend(chunk.samples);
                }
            },
            Ok(CaptureMessage::Pause) => {
                paused = true;
                mic_level = 0.0;
                system_level = 0.0;
                mic_target = 0.0;
                system_target = 0.0;
                let _ = app.emit(
                    "audio-levels",
                    AudioLevels {
                        mic: 0.0,
                        system: 0.0,
                    },
                );
            }
            Ok(CaptureMessage::Resume) => paused = false,
            Ok(CaptureMessage::Warning(message)) => {
                warning = Some(message.clone());
                let _ = app.emit("capture-warning", message);
            }
            Ok(CaptureMessage::Stop) => {
                let _ = app.emit(
                    "audio-levels",
                    AudioLevels {
                        mic: 0.0,
                        system: 0.0,
                    },
                );
                let before = segment.len();
                drain_frames(
                    mode,
                    &mut microphone,
                    &mut system,
                    &mut segment,
                    true,
                    #[cfg(feature = "aec3")]
                    &echo_processor,
                );
                playable_samples += (segment.len() - before) as u64;
                flush_segment(&mut segment, &journal_path, &key, true)?;
                break;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Ok(CaptureMessage::Samples(_)) => {}
        }

        if !paused && last_meter_emit.elapsed() >= METER_EMIT_INTERVAL {
            let now = Instant::now();
            if last_mic_sample.is_none_or(|last| now.duration_since(last) > METER_STALE_AFTER) {
                mic_target = 0.0;
            }
            if last_system_sample.is_none_or(|last| now.duration_since(last) > METER_STALE_AFTER) {
                system_target = 0.0;
            }
            mic_level = smooth_meter(mic_level, mic_target);
            system_level = smooth_meter(system_level, system_target);
            let _ = app.emit(
                "audio-levels",
                AudioLevels {
                    mic: mic_level,
                    system: system_level,
                },
            );
            last_meter_emit = now;
        }
        let before = segment.len();
        drain_frames(
            mode,
            &mut microphone,
            &mut system,
            &mut segment,
            false,
            #[cfg(feature = "aec3")]
            &echo_processor,
        );
        playable_samples += (segment.len() - before) as u64;
        while segment.len() >= SEGMENT_SAMPLES {
            let tail = segment.split_off(SEGMENT_SAMPLES);
            flush_segment(&mut segment, &journal_path, &key, false)?;
            segment = tail;
        }
        if last_disk_check.elapsed() >= Duration::from_secs(2) {
            last_disk_check = Instant::now();
            let available = journal_path
                .parent()
                .and_then(|parent| fs2::available_space(parent).ok())
                .unwrap_or(u64::MAX);
            if available < 200 * 1024 * 1024 && !low_disk_warning_sent {
                let message = "Storage is running low. Stop soon to finalize safely.".to_string();
                warning = Some(message.clone());
                let _ = app.emit("capture-warning", message);
                low_disk_warning_sent = true;
            } else if available >= 300 * 1024 * 1024 {
                low_disk_warning_sent = false;
            }
        }
    }

    if !journal_path.exists() {
        return Err(AppError::Audio("no audio samples were captured".into()));
    }
    let packets = Vault::read_journal(&journal_path, &key)?;
    let m4a = build_m4a(&packets)?;
    let _ = fs::remove_file(&journal_path);
    Ok(WorkerResult {
        m4a,
        playable_ms: ((playable_samples * 1000) / SAMPLE_RATE as u64) as i64,
        warning,
    })
}

fn drain_frames(
    mode: CaptureMode,
    microphone: &mut VecDeque<f32>,
    system: &mut VecDeque<f32>,
    output: &mut Vec<f32>,
    finishing: bool,
    #[cfg(feature = "aec3")] echo_processor: &webrtc_audio_processing::Processor,
) {
    loop {
        let mic_ready = microphone.len() >= FRAME_SAMPLES;
        let system_ready = system.len() >= FRAME_SAMPLES;
        let can_fill_missing =
            finishing || microphone.len() > FRAME_SAMPLES * 10 || system.len() > FRAME_SAMPLES * 10;
        if mode == CaptureMode::InPerson && !mic_ready {
            break;
        }
        if mode == CaptureMode::Online && !(mic_ready && system_ready) && !can_fill_missing {
            break;
        }
        if !mic_ready && !system_ready {
            break;
        }

        let mut mic_frame = take_frame(microphone);
        let system_frame = if mode == CaptureMode::Online {
            take_frame(system)
        } else {
            vec![0.0; FRAME_SAMPLES]
        };

        #[cfg(feature = "aec3")]
        if mode == CaptureMode::Online {
            let mut render = vec![system_frame.clone()];
            let mut capture = vec![mic_frame];
            if echo_processor.process_render_frame(&mut render).is_ok()
                && echo_processor.process_capture_frame(&mut capture).is_ok()
            {
                mic_frame = capture.remove(0);
            } else {
                mic_frame = adaptive_echo_suppression(&mic_frame, &system_frame);
            }
        }
        #[cfg(not(feature = "aec3"))]
        if mode == CaptureMode::Online {
            mic_frame = adaptive_echo_suppression(&mic_frame, &system_frame);
        }

        for index in 0..FRAME_SAMPLES {
            let mixed = if mode == CaptureMode::Online {
                mic_frame[index] * 0.78 + system_frame[index] * 0.78
            } else {
                mic_frame[index]
            };
            output.push(soft_limit(mixed));
        }
    }
}

fn take_frame(queue: &mut VecDeque<f32>) -> Vec<f32> {
    (0..FRAME_SAMPLES)
        .map(|_| queue.pop_front().unwrap_or(0.0))
        .collect()
}

fn adaptive_echo_suppression(microphone: &[f32], system: &[f32]) -> Vec<f32> {
    let cross = microphone
        .iter()
        .zip(system)
        .map(|(mic, sys)| mic * sys)
        .sum::<f32>();
    let system_energy = system
        .iter()
        .map(|sample| sample * sample)
        .sum::<f32>()
        .max(1e-6);
    let coefficient = (cross / system_energy).clamp(0.0, 0.75);
    microphone
        .iter()
        .zip(system)
        .map(|(mic, sys)| mic - sys * coefficient)
        .collect()
}

fn soft_limit(sample: f32) -> f32 {
    if sample.abs() <= 0.92 {
        sample
    } else {
        (sample * 1.35).tanh() * 0.96
    }
}

fn flush_segment(segment: &mut Vec<f32>, path: &Path, key: &[u8; 32], pad: bool) -> AppResult<()> {
    if segment.is_empty() {
        return Ok(());
    }
    if pad {
        let remainder = segment.len() % 1024;
        if remainder != 0 {
            segment.resize(segment.len() + 1024 - remainder, 0.0);
        }
    }
    let mut encoder = AacEncoder::new(AacEncoderConfig {
        bitrate_bps: 96_000,
        ..Default::default()
    });
    encoder
        .push_pcm(segment, 1, SAMPLE_RATE)
        .map_err(|error| AppError::Audio(error.to_string()))?;
    encoder.finish();
    while let Ok(packet) = encoder.next_packet() {
        Vault::append_journal_packet(path, key, &packet.data)?;
    }
    segment.clear();
    Ok(())
}

pub fn build_m4a(packets: &[Vec<u8>]) -> AppResult<Vec<u8>> {
    let config = Mp4Config {
        major_brand: "M4A "
            .parse()
            .map_err(|error| AppError::Audio(format!("invalid M4A brand: {error}")))?,
        minor_version: 0,
        compatible_brands: vec![
            "M4A ".parse().unwrap(),
            "isom".parse().unwrap(),
            "mp42".parse().unwrap(),
        ],
        timescale: 1000,
    };
    let cursor = Cursor::new(Vec::new());
    let mut writer = Mp4Writer::write_start(cursor, &config)
        .map_err(|error| AppError::Audio(error.to_string()))?;
    writer
        .add_track(&TrackConfig {
            track_type: TrackType::Audio,
            timescale: 1000,
            language: "und".into(),
            media_conf: MediaConfig::AacConfig(AacConfig {
                bitrate: 96_000,
                profile: AudioObjectType::AacLowComplexity,
                freq_index: SampleFreqIndex::Freq48000,
                chan_conf: ChannelConfig::Mono,
            }),
        })
        .map_err(|error| AppError::Audio(error.to_string()))?;
    for (index, packet) in packets.iter().enumerate() {
        let start = (index as u64 * 1024 * 1000) / SAMPLE_RATE as u64;
        let end = ((index as u64 + 1) * 1024 * 1000) / SAMPLE_RATE as u64;
        writer
            .write_sample(
                1,
                &Mp4Sample {
                    start_time: start,
                    duration: (end - start) as u32,
                    rendering_offset: 0,
                    is_sync: true,
                    bytes: Bytes::copy_from_slice(packet),
                },
            )
            .map_err(|error| AppError::Audio(error.to_string()))?;
    }
    writer
        .write_end()
        .map_err(|error| AppError::Audio(error.to_string()))?;
    Ok(writer.into_writer().into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_keeps_expected_duration() {
        let source = vec![0.25; 44_100];
        assert_eq!(linear_resample(&source, 44_100, 48_000).len(), 48_000);
    }

    #[test]
    fn fallback_echo_suppression_reduces_correlated_energy() {
        let system: Vec<f32> = (0..FRAME_SAMPLES)
            .map(|index| (index as f32 / 8.0).sin() * 0.4)
            .collect();
        let microphone: Vec<f32> = system.iter().map(|sample| sample * 0.5).collect();
        let cleaned = adaptive_echo_suppression(&microphone, &system);
        let before: f32 = microphone.iter().map(|sample| sample * sample).sum();
        let after: f32 = cleaned.iter().map(|sample| sample * sample).sum();
        assert!(after < before * 0.1);
    }

    #[test]
    fn meter_uses_a_perceptual_audio_scale() {
        assert_eq!(meter_level(&[0.0; 480]), 0.0);
        let quiet_speech = meter_level(&[0.01; 480]);
        let loud_speech = meter_level(&[0.1; 480]);
        assert!(quiet_speech > 0.35, "quiet speech should be visible");
        assert!(loud_speech > quiet_speech);
        assert_eq!(meter_level(&[1.0; 480]), 1.0);
    }

    #[test]
    fn meter_smoothing_attacks_faster_than_it_releases() {
        let attack = smooth_meter(0.0, 1.0);
        let release = smooth_meter(1.0, 0.0);
        assert!(attack > 0.5);
        assert!(release > attack);
        assert_eq!(smooth_meter(0.004, 0.0), 0.0);
    }

    #[test]
    fn aac_packets_mux_to_m4a() {
        let samples: Vec<f32> = (0..48_000)
            .map(|index| (index as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.15)
            .collect();
        let mut encoder = AacEncoder::new(AacEncoderConfig {
            bitrate_bps: 96_000,
            ..Default::default()
        });
        encoder.push_pcm(&samples, 1, SAMPLE_RATE).unwrap();
        encoder.finish();
        let mut packets = Vec::new();
        while let Ok(packet) = encoder.next_packet() {
            packets.push(packet.data);
        }
        let m4a = build_m4a(&packets).unwrap();
        assert!(m4a.windows(4).any(|window| window == b"ftyp"));
        assert!(m4a.windows(4).any(|window| window == b"moov"));
    }
}
