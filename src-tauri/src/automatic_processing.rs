//! Runs opt-in processing in the native app, independent of open webviews.
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    models::{AppSettings, Recording},
    state::AppState,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomaticProcessingStatus {
    pub recording_id: String,
    pub title: String,
    pub kind: &'static str,
    pub active: bool,
    pub error: Option<String>,
}

// A transcript version prevents queued work from summarizing a replacement
// transcript twice. Settings are checked again when a waiting job starts.
fn should_run(
    settings: &AppSettings,
    recording: &Recording,
    transcript_version: Option<&str>,
) -> bool {
    match transcript_version {
        None => settings.auto_transcribe && recording.transcript.is_none(),
        Some(version) => {
            settings.auto_summarize
                && recording.transcript.as_ref().is_some_and(|transcript| {
                    transcript.created_at == version && !transcript.text.trim().is_empty()
                })
        }
    }
}

pub fn enqueue(app: &AppHandle, id: &str, transcript_version: Option<String>) {
    let app = app.clone();
    let id = id.to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        // Automatic work waits for manual work and other automatic jobs. Keep
        // model memory bounded and never run two paid requests concurrently.
        let Ok(guard) = state.processing_lock.lock() else {
            return;
        };
        let Ok(settings) = state.repository.settings() else {
            return;
        };
        let Ok(recording) = state.repository.recording(&id) else {
            return;
        };
        if !should_run(&settings, &recording, transcript_version.as_deref()) {
            return;
        }
        let mut status = AutomaticProcessingStatus {
            recording_id: id.clone(),
            title: recording.title,
            kind: if transcript_version.is_some() {
                "summary"
            } else {
                "transcription"
            },
            active: true,
            error: None,
        };
        publish(&app, &state, &status);
        let result = if transcript_version.is_some() {
            state.summarize_recording_inner(&app, &id, None, None)
        } else {
            state.transcribe_recording_inner(&app, &id, None)
        };
        status.active = false;
        match &result {
            Ok(updated) => {
                let _ = app.emit("recording-updated", updated);
            }
            Err(error) => {
                status.error = Some(error.to_string());
            }
        }
        publish(&app, &state, &status);
        drop(guard);
        // A failed transcription never starts a summary. Failures are shown to
        // the user, with no automatic retry or changes to existing content.
        if transcript_version.is_none() {
            if let Ok(updated) = result {
                after_transcription(&app, &updated);
            }
        }
    });
}

pub fn after_transcription(app: &AppHandle, recording: &Recording) {
    let state = app.state::<AppState>();
    if state
        .repository
        .settings()
        .is_ok_and(|settings| settings.auto_summarize)
    {
        if let Some(transcript) = &recording.transcript {
            enqueue(app, &recording.id, Some(transcript.created_at.clone()));
        }
    }
}

fn publish(app: &AppHandle, state: &AppState, status: &AutomaticProcessingStatus) {
    if let Ok(mut current) = state.automatic_status.lock() {
        *current = Some(status.clone());
    }
    let _ = app.emit("automatic-processing", status);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CaptureMode, Transcript};

    #[test]
    fn automatic_processing_is_opt_in_and_skips_stale_or_empty_transcripts() {
        let mut settings = AppSettings::default();
        let mut recording = Recording {
            id: "one".into(),
            title: "Recording".into(),
            mode: CaptureMode::Online,
            started_at: "now".into(),
            ended_at: Some("now".into()),
            duration_ms: 1000,
            playable_duration_ms: 1000,
            status: "ready".into(),
            size_bytes: 100,
            codec: "AAC-LC".into(),
            detected_app: None,
            deleted_at: None,
            highlights: vec![],
            transcript: None,
            summary: None,
        };
        assert!(!should_run(&settings, &recording, None));
        settings.auto_transcribe = true;
        assert!(should_run(&settings, &recording, None));
        assert!(!should_run(&settings, &recording, Some("v1")));
        recording.transcript = Some(Transcript {
            text: "Speech".into(),
            created_at: "v1".into(),
            language: None,
            segments: vec![],
            source: None,
            model: None,
        });
        assert!(!should_run(&settings, &recording, None));
        assert!(!should_run(&settings, &recording, Some("v1")));
        settings.auto_summarize = true;
        settings.auto_transcribe = false; // manual transcripts can auto summarize
        assert!(should_run(&settings, &recording, Some("v1")));
        assert!(!should_run(&settings, &recording, Some("old")));
        recording.transcript.as_mut().unwrap().text.clear();
        assert!(!should_run(&settings, &recording, Some("v1")));
        settings.auto_summarize = false;
        assert!(!should_run(&settings, &recording, Some("v1")));
    }
}
