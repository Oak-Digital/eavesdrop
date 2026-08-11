use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ProjectRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ProjectPage {
    pub projects: Vec<ProjectRef>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DeliveryMetadata {
    pub recording_id: String,
    pub title: String,
    pub started_at: String,
    pub duration_ms: i64,
    pub highlights_ms: Vec<i64>,
    pub source: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum DeliveryFailureCode {
    AuthenticationRequired,
    PermissionDenied,
    NetworkUnavailable,
    RateLimited,
    RemoteRejected,
    Cancelled,
    InvalidRequest,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DeliveryFailure {
    pub code: DeliveryFailureCode,
    pub message: String,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct UploadProgress {
    pub bytes_sent: u64,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct UploadResult {
    pub remote_asset_id: String,
}

#[allow(dead_code)]
pub trait RecordingDestination: Send + Sync {
    fn id(&self) -> &'static str;
    fn connection_status(&self) -> ConnectionStatus;
    fn connect(&self) -> Result<(), DeliveryFailure>;
    fn disconnect(&self) -> Result<(), DeliveryFailure>;
    fn list_projects(
        &self,
        query: &str,
        cursor: Option<&str>,
    ) -> Result<ProjectPage, DeliveryFailure>;
    fn upload(
        &self,
        project: Option<&ProjectRef>,
        metadata: &DeliveryMetadata,
        decrypted_m4a: &mut dyn Read,
        total_bytes: Option<u64>,
        cancelled: &AtomicBool,
        progress: &mut dyn FnMut(UploadProgress),
    ) -> Result<UploadResult, DeliveryFailure>;
}

pub struct LocalExportDestination;

impl RecordingDestination for LocalExportDestination {
    fn id(&self) -> &'static str {
        "local_export"
    }

    fn connection_status(&self) -> ConnectionStatus {
        ConnectionStatus::Connected
    }

    fn connect(&self) -> Result<(), DeliveryFailure> {
        Ok(())
    }

    fn disconnect(&self) -> Result<(), DeliveryFailure> {
        Ok(())
    }

    fn list_projects(&self, _: &str, _: Option<&str>) -> Result<ProjectPage, DeliveryFailure> {
        Ok(ProjectPage {
            projects: Vec::new(),
            next_cursor: None,
        })
    }

    fn upload(
        &self,
        _: Option<&ProjectRef>,
        metadata: &DeliveryMetadata,
        decrypted_m4a: &mut dyn Read,
        total_bytes: Option<u64>,
        cancelled: &AtomicBool,
        progress: &mut dyn FnMut(UploadProgress),
    ) -> Result<UploadResult, DeliveryFailure> {
        let mut consumed = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            if cancelled.load(Ordering::Relaxed) {
                return Err(cancelled_failure());
            }
            let count = decrypted_m4a.read(&mut buffer).map_err(io_failure)?;
            if count == 0 {
                break;
            }
            consumed += count as u64;
            progress(UploadProgress {
                bytes_sent: consumed,
                total_bytes,
            });
        }
        Ok(UploadResult {
            remote_asset_id: format!("local:{}", metadata.recording_id),
        })
    }
}

impl LocalExportDestination {
    pub fn export(path: &Path, decrypted_m4a: &[u8]) -> AppResult<()> {
        let mut file = File::create(path)?;
        file.write_all(decrypted_m4a)?;
        file.sync_all()?;
        Ok(())
    }
}

/// Deterministic in-memory adapter used by destination contract tests and future
/// project-system UI work. It remains available in development builds so an
/// integration can be exercised without a backend account.
#[derive(Default)]
#[allow(dead_code)]
pub struct FakeDestination {
    connected: AtomicBool,
    fail_next: Mutex<Option<DeliveryFailure>>,
    uploads: Arc<Mutex<FakeUploads>>,
}

type FakeUploads = Vec<(DeliveryMetadata, Vec<u8>)>;

#[allow(dead_code)]
impl FakeDestination {
    pub fn fail_once(&self, failure: DeliveryFailure) {
        *self.fail_next.lock().expect("fake destination lock") = Some(failure);
    }

    pub fn uploads(&self) -> Vec<(DeliveryMetadata, Vec<u8>)> {
        self.uploads.lock().expect("fake destination lock").clone()
    }
}

impl RecordingDestination for FakeDestination {
    fn id(&self) -> &'static str {
        "fake"
    }

    fn connection_status(&self) -> ConnectionStatus {
        if self.connected.load(Ordering::Relaxed) {
            ConnectionStatus::Connected
        } else {
            ConnectionStatus::Disconnected
        }
    }

    fn connect(&self) -> Result<(), DeliveryFailure> {
        self.connected.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn disconnect(&self) -> Result<(), DeliveryFailure> {
        self.connected.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn list_projects(
        &self,
        query: &str,
        cursor: Option<&str>,
    ) -> Result<ProjectPage, DeliveryFailure> {
        if !self.connected.load(Ordering::Relaxed) {
            return Err(authentication_failure());
        }
        let offset = cursor
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        let projects = ["Product", "Pilot", "Research"];
        let filtered: Vec<_> = projects
            .iter()
            .enumerate()
            .filter(|(_, name)| name.to_lowercase().contains(&query.to_lowercase()))
            .skip(offset)
            .take(2)
            .map(|(index, name)| ProjectRef {
                id: format!("project-{index}"),
                name: (*name).into(),
            })
            .collect();
        let next_cursor = (filtered.len() == 2).then(|| (offset + 2).to_string());
        Ok(ProjectPage {
            projects: filtered,
            next_cursor,
        })
    }

    fn upload(
        &self,
        _: Option<&ProjectRef>,
        metadata: &DeliveryMetadata,
        decrypted_m4a: &mut dyn Read,
        total_bytes: Option<u64>,
        cancelled: &AtomicBool,
        progress: &mut dyn FnMut(UploadProgress),
    ) -> Result<UploadResult, DeliveryFailure> {
        if !self.connected.load(Ordering::Relaxed) {
            return Err(authentication_failure());
        }
        if let Some(failure) = self.fail_next.lock().expect("fake destination lock").take() {
            return Err(failure);
        }
        let mut output = Vec::new();
        let mut buffer = [0u8; 16];
        loop {
            if cancelled.load(Ordering::Relaxed) {
                return Err(cancelled_failure());
            }
            let count = decrypted_m4a.read(&mut buffer).map_err(io_failure)?;
            if count == 0 {
                break;
            }
            output.extend_from_slice(&buffer[..count]);
            progress(UploadProgress {
                bytes_sent: output.len() as u64,
                total_bytes,
            });
        }
        self.uploads
            .lock()
            .expect("fake destination lock")
            .push((metadata.clone(), output));
        Ok(UploadResult {
            remote_asset_id: format!("fake-{}", metadata.recording_id),
        })
    }
}

#[allow(dead_code)]
fn authentication_failure() -> DeliveryFailure {
    DeliveryFailure {
        code: DeliveryFailureCode::AuthenticationRequired,
        message: "connect the destination before sending".into(),
        retryable: true,
    }
}

#[allow(dead_code)]
fn cancelled_failure() -> DeliveryFailure {
    DeliveryFailure {
        code: DeliveryFailureCode::Cancelled,
        message: "upload cancelled".into(),
        retryable: true,
    }
}

#[allow(dead_code)]
fn io_failure(error: std::io::Error) -> DeliveryFailure {
    DeliveryFailure {
        code: DeliveryFailureCode::Unknown,
        message: error.to_string(),
        retryable: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn metadata() -> DeliveryMetadata {
        DeliveryMetadata {
            recording_id: "recording-1".into(),
            title: "Planning".into(),
            started_at: "2026-08-11T08:00:00Z".into(),
            duration_ms: 1_000,
            highlights_ms: vec![500],
            source: HashMap::new(),
        }
    }

    #[test]
    fn fake_destination_supports_retry_progress_and_streaming() {
        let destination = FakeDestination::default();
        destination.connect().unwrap();
        destination.fail_once(DeliveryFailure {
            code: DeliveryFailureCode::NetworkUnavailable,
            message: "offline".into(),
            retryable: true,
        });
        let cancelled = AtomicBool::new(false);
        let mut first = Cursor::new(vec![1, 2, 3]);
        let error = destination
            .upload(
                None,
                &metadata(),
                &mut first,
                Some(3),
                &cancelled,
                &mut |_| {},
            )
            .unwrap_err();
        assert!(error.retryable);

        let mut progress = Vec::new();
        let mut retry = Cursor::new(vec![1, 2, 3]);
        let result = destination
            .upload(
                None,
                &metadata(),
                &mut retry,
                Some(3),
                &cancelled,
                &mut |item| progress.push(item.bytes_sent),
            )
            .unwrap();
        assert_eq!(result.remote_asset_id, "fake-recording-1");
        assert_eq!(destination.uploads()[0].1, vec![1, 2, 3]);
        assert_eq!(progress.last(), Some(&3));
    }

    #[test]
    fn fake_destination_honors_cancellation() {
        let destination = FakeDestination::default();
        destination.connect().unwrap();
        let cancelled = AtomicBool::new(true);
        let mut audio = Cursor::new(vec![1, 2, 3]);
        let error = destination
            .upload(
                None,
                &metadata(),
                &mut audio,
                Some(3),
                &cancelled,
                &mut |_| {},
            )
            .unwrap_err();
        assert_eq!(error.code, DeliveryFailureCode::Cancelled);
        assert!(destination.uploads().is_empty());
    }
}
