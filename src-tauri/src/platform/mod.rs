#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod fallback;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use crate::audio::CaptureBackend;

#[cfg(target_os = "macos")]
pub fn capture_backend() -> Box<dyn CaptureBackend> {
    Box::new(macos::MacCaptureBackend::default())
}

#[cfg(target_os = "windows")]
pub fn capture_backend() -> Box<dyn CaptureBackend> {
    Box::new(windows::WindowsCaptureBackend::default())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn capture_backend() -> Box<dyn CaptureBackend> {
    Box::new(fallback::FallbackCaptureBackend::default())
}

pub fn permission_state(request: bool) -> crate::models::PermissionState {
    #[cfg(target_os = "macos")]
    {
        macos::permission_state(request)
    }
    #[cfg(target_os = "windows")]
    {
        windows::permission_state(request)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        fallback::permission_state(request)
    }
}
