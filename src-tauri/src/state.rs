use std::{fs, path::Path, sync::Mutex};

use chrono::{Local, Utc};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    audio::{self, CaptureSession},
    crypto::{RecordingKey, Vault},
    error::{AppError, AppResult},
    models::{
        AppSnapshot, CaptureMode, Recording, RecordingPhase, RecordingSession, SettingsPatch,
        StartRecordingInput,
    },
    platform,
    storage::Repository,
};

pub struct AppState {
    pub repository: Repository,
    pub vault: Vault,
    runtime: Mutex<RuntimeState>,
}

struct RuntimeState {
    phase: RecordingPhase,
    active: Option<ActiveRecording>,
    last_ready_id: Option<String>,
    error: Option<String>,
    update_installing: bool,
}

struct ActiveRecording {
    id: String,
    mode: CaptureMode,
    started_at: String,
    key: RecordingKey,
    capture: CaptureSession,
}

impl AppState {
    pub fn open(app: &AppHandle) -> AppResult<Self> {
        let app_data = app
            .path()
            .app_data_dir()
            .map_err(|error| AppError::Storage(error.to_string()))?;
        fs::create_dir_all(&app_data)?;
        let repository = Repository::open(&app_data.join("library.sqlite3"))?;
        let vault = Vault::open(app_data.join("recordings"))?;
        let state = Self {
            repository,
            vault,
            runtime: Mutex::new(RuntimeState {
                phase: RecordingPhase::Idle,
                active: None,
                last_ready_id: None,
                error: None,
                update_installing: false,
            }),
        };
        state.recover_unfinished()?;
        for path in state.repository.purge_expired()? {
            let _ = fs::remove_file(path);
        }
        Ok(state)
    }

    pub fn snapshot(&self, request_permissions: bool) -> AppResult<AppSnapshot> {
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| AppError::State("recording lock poisoned".into()))?;
        let session = if let Some(active) = runtime.active.as_ref() {
            RecordingSession {
                phase: runtime.phase.clone(),
                recording_id: Some(active.id.clone()),
                mode: Some(active.mode),
                started_at: Some(active.started_at.clone()),
                elapsed_ms: active.capture.elapsed_ms(),
                playable_ms: active.capture.playable_ms(),
                mic_level: 0.0,
                system_level: 0.0,
                warning: None,
                error: runtime.error.clone(),
            }
        } else {
            RecordingSession {
                phase: runtime.phase.clone(),
                recording_id: runtime.last_ready_id.clone(),
                error: runtime.error.clone(),
                ..Default::default()
            }
        };
        Ok(AppSnapshot {
            session,
            permissions: platform::permission_state(request_permissions),
            settings: self.repository.settings()?,
            devices: audio::input_devices(),
        })
    }

    pub fn start(&self, app: AppHandle, input: StartRecordingInput) -> AppResult<RecordingSession> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| AppError::State("recording lock poisoned".into()))?;
        ensure_recording_can_start(&runtime)?;
        runtime.phase = RecordingPhase::Starting;
        runtime.error = None;
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let started_at = now.to_rfc3339();
        let title = default_title(input.mode, input.detected_app.as_deref());
        let key = self.vault.create_recording_key()?;
        let journal_path = self.vault.journal_path(&id);
        if fs2::available_space(journal_path.parent().unwrap_or(Path::new("."))).unwrap_or(u64::MAX)
            < 500 * 1024 * 1024
        {
            runtime.phase = RecordingPhase::Failed;
            return Err(AppError::Storage(
                "at least 500 MB of free space is required to start".into(),
            ));
        }
        let placeholder = Recording {
            id: id.clone(),
            title,
            mode: input.mode,
            started_at: started_at.clone(),
            ended_at: None,
            duration_ms: 0,
            playable_duration_ms: 0,
            status: "recording".into(),
            size_bytes: 0,
            codec: "AAC-LC".into(),
            detected_app: input.detected_app,
            deleted_at: None,
            highlights: Vec::new(),
        };
        self.repository.insert_recording(
            &placeholder,
            &journal_path.to_string_lossy(),
            &key.wrapped,
        )?;
        let microphone_id = self.repository.settings()?.microphone_id;
        match CaptureSession::start(
            app.clone(),
            input.mode,
            microphone_id.as_deref(),
            journal_path,
            key.plain,
        ) {
            Ok(capture) => {
                runtime.phase = RecordingPhase::Recording;
                runtime.active = Some(ActiveRecording {
                    id,
                    mode: input.mode,
                    started_at,
                    key,
                    capture,
                });
                crate::update_tray(&app, true);
                let session = session_from_runtime(&runtime);
                let _ = app.emit("recording-state-changed", &session);
                Ok(session)
            }
            Err(error) => {
                runtime.phase = RecordingPhase::Failed;
                runtime.error = Some(error.to_string());
                crate::update_tray(&app, false);
                Err(error)
            }
        }
    }

    pub fn pause(&self, app: &AppHandle) -> AppResult<RecordingSession> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| AppError::State("recording lock poisoned".into()))?;
        let active = runtime
            .active
            .as_mut()
            .ok_or_else(|| AppError::State("no recording is active".into()))?;
        active.capture.pause()?;
        runtime.phase = RecordingPhase::Paused;
        emit_session(app, &runtime)
    }

    pub fn resume(&self, app: &AppHandle) -> AppResult<RecordingSession> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| AppError::State("recording lock poisoned".into()))?;
        let active = runtime
            .active
            .as_mut()
            .ok_or_else(|| AppError::State("no recording is active".into()))?;
        active.capture.resume()?;
        runtime.phase = RecordingPhase::Recording;
        emit_session(app, &runtime)
    }

    pub fn highlight(&self, app: &AppHandle) -> AppResult<RecordingSession> {
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| AppError::State("recording lock poisoned".into()))?;
        let active = runtime
            .active
            .as_ref()
            .ok_or_else(|| AppError::State("no recording is active".into()))?;
        if active.capture.is_paused() {
            return Err(AppError::State("resume before adding a highlight".into()));
        }
        self.repository
            .add_highlight(&active.id, active.capture.playable_ms())?;
        emit_session(app, &runtime)
    }

    pub fn stop(&self, app: &AppHandle) -> AppResult<Recording> {
        let (active, elapsed_ms) = {
            let mut runtime = self
                .runtime
                .lock()
                .map_err(|_| AppError::State("recording lock poisoned".into()))?;
            runtime.phase = RecordingPhase::Finalizing;
            let active = runtime
                .active
                .take()
                .ok_or_else(|| AppError::State("no recording is active".into()))?;
            let elapsed = active.capture.elapsed_ms();
            let _ = app.emit("recording-state-changed", session_from_runtime(&runtime));
            (active, elapsed)
        };
        let id = active.id.clone();
        let output = active.capture.stop()?;
        let asset_path = self.vault.seal_asset(&id, &active.key.plain, &output.m4a)?;
        self.repository.finalize_recording(
            &id,
            &Utc::now().to_rfc3339(),
            elapsed_ms,
            output.playable_ms,
            output.m4a.len() as i64,
            &asset_path.to_string_lossy(),
            false,
        )?;
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| AppError::State("recording lock poisoned".into()))?;
        runtime.phase = RecordingPhase::Ready;
        crate::update_tray(app, false);
        runtime.last_ready_id = Some(id.clone());
        runtime.error = output.warning;
        let _ = app.emit("recording-state-changed", session_from_runtime(&runtime));
        let recording = self.repository.recording(&id)?;
        let _ = app.emit("recording-finalized", &recording);
        Ok(recording)
    }

    pub fn decrypt_recording(&self, id: &str) -> AppResult<Vec<u8>> {
        let secrets = self.repository.secrets(id)?;
        let path = secrets
            .asset_path
            .ok_or_else(|| AppError::State("recording is not finalized".into()))?;
        let key = self.vault.unwrap_key(&secrets.wrapped_key)?;
        self.vault.open_asset(Path::new(&path), &key)
    }

    pub fn update_settings(&self, patch: &SettingsPatch) -> AppResult<crate::models::AppSettings> {
        self.repository.update_settings(patch)
    }

    pub fn is_recording(&self) -> bool {
        self.runtime
            .lock()
            .map(|runtime| runtime.active.is_some())
            .unwrap_or(true)
    }

    pub fn begin_update_install(&self) -> AppResult<()> {
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| AppError::State("recording lock poisoned".into()))?;
        if runtime.active.is_some() {
            return Err(AppError::State(
                "stop the active recording before installing the update".into(),
            ));
        }
        runtime.update_installing = true;
        Ok(())
    }

    pub fn cancel_update_install(&self) {
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime.update_installing = false;
        }
    }

    fn recover_unfinished(&self) -> AppResult<()> {
        for id in self.repository.unfinished_recordings()? {
            let secrets = self.repository.secrets(&id)?;
            let Some(journal_path) = secrets.journal_path else {
                continue;
            };
            if !Path::new(&journal_path).exists() {
                continue;
            }
            let key = self.vault.unwrap_key(&secrets.wrapped_key)?;
            let packets = Vault::read_journal(Path::new(&journal_path), &key)?;
            if packets.is_empty() {
                continue;
            }
            let m4a = audio::build_m4a(&packets)?;
            let asset_path = self.vault.seal_asset(&id, &key, &m4a)?;
            let playable_ms =
                ((packets.len() as u64 * 1024 * 1000) / audio::SAMPLE_RATE as u64) as i64;
            self.repository.finalize_recording(
                &id,
                &Utc::now().to_rfc3339(),
                playable_ms,
                playable_ms,
                m4a.len() as i64,
                &asset_path.to_string_lossy(),
                true,
            )?;
            let _ = fs::remove_file(journal_path);
        }
        Ok(())
    }
}

fn default_title(mode: CaptureMode, detected_app: Option<&str>) -> String {
    let prefix = detected_app.unwrap_or(match mode {
        CaptureMode::Online => "Online meeting",
        CaptureMode::InPerson => "In-person meeting",
    });
    format!("{prefix} — {}", Local::now().format("%d %b %Y, %H:%M"))
}

fn session_from_runtime(runtime: &RuntimeState) -> RecordingSession {
    if let Some(active) = runtime.active.as_ref() {
        RecordingSession {
            phase: runtime.phase.clone(),
            recording_id: Some(active.id.clone()),
            mode: Some(active.mode),
            started_at: Some(active.started_at.clone()),
            elapsed_ms: active.capture.elapsed_ms(),
            playable_ms: active.capture.playable_ms(),
            mic_level: 0.0,
            system_level: 0.0,
            warning: None,
            error: runtime.error.clone(),
        }
    } else {
        RecordingSession {
            phase: runtime.phase.clone(),
            recording_id: runtime.last_ready_id.clone(),
            error: runtime.error.clone(),
            ..Default::default()
        }
    }
}

fn emit_session(app: &AppHandle, runtime: &RuntimeState) -> AppResult<RecordingSession> {
    let session = session_from_runtime(runtime);
    app.emit("recording-state-changed", &session)
        .map_err(|error| AppError::Other(error.to_string()))?;
    Ok(session)
}

fn ensure_recording_can_start(runtime: &RuntimeState) -> AppResult<()> {
    if runtime.update_installing {
        return Err(AppError::State(
            "an application update is being installed".into(),
        ));
    }
    if runtime.active.is_some()
        || !matches!(
            runtime.phase,
            RecordingPhase::Idle | RecordingPhase::Ready | RecordingPhase::Failed
        )
    {
        return Err(AppError::State("a recording is already active".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_install_lock_blocks_new_recordings() {
        let mut runtime = RuntimeState {
            phase: RecordingPhase::Idle,
            active: None,
            last_ready_id: None,
            error: None,
            update_installing: false,
        };
        assert!(ensure_recording_can_start(&runtime).is_ok());
        runtime.update_installing = true;
        assert!(ensure_recording_can_start(&runtime).is_err());
    }
}
