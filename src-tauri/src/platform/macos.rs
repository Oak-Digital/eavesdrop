use std::{
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};

use cpal::{Stream, traits::HostTrait};
use screencapturekit::prelude::*;

use crate::{
    audio::{CaptureBackend, CaptureMessage, SourceKind, send_pcm, start_microphone_stream},
    error::{AppError, AppResult},
    models::{CaptureMode, PermissionState, PermissionValue},
};

#[derive(Default)]
pub struct MacCaptureBackend {
    microphone: Option<Stream>,
    system_audio: Option<SCStream>,
}

impl CaptureBackend for MacCaptureBackend {
    fn start(
        &mut self,
        mode: CaptureMode,
        microphone_id: Option<&str>,
        sender: Sender<CaptureMessage>,
    ) -> AppResult<()> {
        self.microphone = Some(start_microphone_stream(microphone_id, sender.clone())?);
        if mode == CaptureMode::Online {
            let content = SCShareableContent::get().map_err(|error| {
                AppError::Permission(format!(
                    "Screen Recording permission is required for computer audio. Enable Eavesdrop in System Settings, then quit and reopen it. ({error})"
                ))
            })?;
            let display = content.displays().into_iter().next().ok_or_else(|| {
                AppError::Audio("no display is available for computer audio capture".into())
            })?;
            let filter = SCContentFilter::create()
                .with_display(&display)
                .with_excluding_windows(&[])
                .build();
            let config = SCStreamConfiguration::new()
                .with_width(2)
                .with_height(2)
                .with_captures_audio(true)
                .with_excludes_current_process_audio(true)
                .with_sample_rate(48_000)
                .with_channel_count(1);
            let mut stream = SCStream::new(&filter, &config);
            let audio_sender = sender.clone();
            stream.add_output_handler(
                move |sample: CMSampleBuffer, output_type| {
                    if output_type != SCStreamOutputType::Audio {
                        return;
                    }
                    let Some(buffers) = sample.audio_buffer_list() else {
                        return;
                    };
                    for buffer in buffers.iter() {
                        let samples: Vec<f32> = buffer
                            .data()
                            .chunks_exact(4)
                            .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
                            .collect();
                        send_pcm(
                            &audio_sender,
                            SourceKind::System,
                            &samples,
                            buffer.number_channels.max(1) as usize,
                            48_000,
                        );
                    }
                },
                SCStreamOutputType::Audio,
            );
            stream.start_capture().map_err(|error| {
                AppError::Permission(format!("computer audio permission is required: {error}"))
            })?;
            self.system_audio = Some(stream);
        }
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(stream) = self.system_audio.as_mut() {
            let _ = stream.stop_capture();
        }
        self.system_audio.take();
        self.microphone.take();
    }
}

pub fn permission_state(request: bool) -> PermissionState {
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    let system_granted = unsafe {
        if request {
            CGRequestScreenCaptureAccess()
        } else {
            CGPreflightScreenCaptureAccess()
        }
    };
    let microphone = if cpal::default_host().default_input_device().is_none() {
        PermissionValue::Unavailable
    } else if request {
        // CoreAudio presents macOS' microphone consent sheet when the first input
        // stream starts. Hold the probe briefly so the system can register it.
        let (sender, _receiver) = mpsc::channel();
        match start_microphone_stream(None, sender) {
            Ok(stream) => {
                thread::sleep(Duration::from_millis(150));
                drop(stream);
                PermissionValue::Granted
            }
            Err(_) => PermissionValue::Denied,
        }
    } else {
        // CoreAudio does not expose a stable cross-version preflight through
        // cpal; availability is reported here and the explicit probe above is
        // authoritative when onboarding asks for access.
        PermissionValue::Granted
    };
    PermissionState {
        microphone,
        system_audio: if system_granted {
            PermissionValue::Granted
        } else if request {
            PermissionValue::Denied
        } else {
            PermissionValue::NotDetermined
        },
    }
}
