use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
    },
    thread::{self, JoinHandle},
};

use cpal::{Stream, traits::HostTrait};
use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat, initialize_mta};

use crate::{
    audio::{CaptureBackend, CaptureMessage, SourceKind, send_pcm, start_microphone_stream},
    error::{AppError, AppResult},
    models::{CaptureMode, PermissionState, PermissionValue},
};

#[derive(Default)]
pub struct WindowsCaptureBackend {
    microphone: Option<Stream>,
    loopback_stop: Option<Arc<AtomicBool>>,
    loopback_thread: Option<JoinHandle<()>>,
}

impl CaptureBackend for WindowsCaptureBackend {
    fn start(
        &mut self,
        mode: CaptureMode,
        microphone_id: Option<&str>,
        sender: Sender<CaptureMessage>,
    ) -> AppResult<()> {
        self.microphone = Some(start_microphone_stream(microphone_id, sender.clone())?);
        self.set_system_audio(mode == CaptureMode::Online, sender)
    }

    fn set_system_audio(&mut self, enabled: bool, sender: Sender<CaptureMessage>) -> AppResult<()> {
        if enabled {
            if self.loopback_thread.is_some() {
                return Ok(());
            }
            let stop = Arc::new(AtomicBool::new(false));
            let thread_stop = stop.clone();
            let thread_sender = sender.clone();
            let (ready_sender, ready_receiver) = mpsc::channel();
            let handle = thread::Builder::new()
                .name("eavesdrop-wasapi-loopback".into())
                .spawn(move || {
                    let mut initialized = false;
                    if let Err(error) = capture_loopback(
                        thread_sender.clone(),
                        thread_stop,
                        &ready_sender,
                        &mut initialized,
                    ) {
                        // Before initialization, report the failure to the
                        // caller so the current source selection stays intact.
                        if initialized {
                            let _ = thread_sender.send(CaptureMessage::Warning(format!(
                                "Computer audio interrupted: {error}"
                            )));
                        } else {
                            let _ = ready_sender.send(Err(error.to_string()));
                        }
                    }
                })
                .map_err(|error| AppError::Audio(error.to_string()))?;
            match ready_receiver.recv() {
                Ok(Ok(())) => {}
                result => {
                    stop.store(true, Ordering::SeqCst);
                    let _ = handle.join();
                    return Err(AppError::Audio(match result {
                        Ok(Err(error)) => error,
                        _ => "computer audio capture did not start".into(),
                    }));
                }
            }
            self.loopback_stop = Some(stop);
            self.loopback_thread = Some(handle);
        } else {
            if let Some(stop) = self.loopback_stop.take() {
                stop.store(true, Ordering::SeqCst);
            }
            if let Some(handle) = self.loopback_thread.take() {
                let _ = handle.join();
            }
        }
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(stop) = self.loopback_stop.take() {
            stop.store(true, Ordering::SeqCst);
        }
        if let Some(handle) = self.loopback_thread.take() {
            let _ = handle.join();
        }
        self.microphone.take();
    }
}

fn capture_loopback(
    sender: Sender<CaptureMessage>,
    stop: Arc<AtomicBool>,
    ready: &Sender<Result<(), String>>,
    initialized: &mut bool,
) -> Result<(), Box<dyn std::error::Error>> {
    initialize_mta().ok()?;
    let enumerator = DeviceEnumerator::new()?;
    let device = enumerator.get_default_device(&Direction::Render)?;
    let mut audio_client = device.get_iaudioclient()?;
    let format = WaveFormat::new(32, 32, &SampleType::Float, 48_000, 1, None);
    let (_, minimum_period) = audio_client.get_device_period()?;
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: minimum_period,
    };
    audio_client.initialize_client(&format, &Direction::Capture, &mode)?;
    let event = audio_client.set_get_eventhandle()?;
    let capture_client = audio_client.get_audiocaptureclient()?;
    let mut bytes = VecDeque::new();
    audio_client.start_stream()?;
    *initialized = true;
    let _ = ready.send(Ok(()));
    while !stop.load(Ordering::Relaxed) {
        capture_client.read_from_device_to_deque(&mut bytes)?;
        let sample_count = bytes.len() / 4;
        if sample_count > 0 {
            let mut samples = Vec::with_capacity(sample_count);
            for _ in 0..sample_count {
                let raw = [
                    bytes.pop_front().unwrap(),
                    bytes.pop_front().unwrap(),
                    bytes.pop_front().unwrap(),
                    bytes.pop_front().unwrap(),
                ];
                samples.push(f32::from_le_bytes(raw));
            }
            send_pcm(&sender, SourceKind::System, &samples, 1, 48_000);
        }
        let _ = event.wait_for_event(200);
    }
    let _ = audio_client.stop_stream();
    Ok(())
}

pub fn permission_state(_: bool) -> PermissionState {
    PermissionState {
        microphone: if cpal::default_host().default_input_device().is_some() {
            PermissionValue::Granted
        } else {
            PermissionValue::Unavailable
        },
        system_audio: PermissionValue::Granted,
    }
}
