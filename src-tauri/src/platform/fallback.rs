use crate::{
    audio::{CaptureBackend, CaptureMessage, start_microphone_stream},
    error::{AppError, AppResult},
    models::{CaptureMode, PermissionState, PermissionValue},
};
use cpal::Stream;
use std::sync::mpsc::Sender;

#[derive(Default)]
pub struct FallbackCaptureBackend {
    microphone: Option<Stream>,
}
impl CaptureBackend for FallbackCaptureBackend {
    fn start(
        &mut self,
        mode: CaptureMode,
        microphone_id: Option<&str>,
        sender: Sender<CaptureMessage>,
    ) -> AppResult<()> {
        if mode == CaptureMode::Online {
            return Err(AppError::Audio(
                "computer audio capture is supported on macOS and Windows".into(),
            ));
        }
        self.microphone = Some(start_microphone_stream(microphone_id, sender)?);
        Ok(())
    }
    fn stop(&mut self) {
        self.microphone.take();
    }
}
pub fn permission_state(_: bool) -> PermissionState {
    PermissionState {
        microphone: PermissionValue::Unavailable,
        system_audio: PermissionValue::Unavailable,
    }
}
