use std::path::Path;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    destinations::LocalExportDestination,
    detection::MeetingDetector,
    diagnostics::Diagnostics,
    error::{AppError, AppResult},
    models::{
        AppSnapshot, ModelDownloadStatus, OnboardingSettings, Recording, RecordingSession,
        SettingsPatch, StartRecordingInput, SummaryModelInfo, WhisperModelInfo,
    },
    oakos::{self, OakOsIntegration, OakOsProject, OakOsPublishResult},
    state::AppState,
};

#[tauri::command]
pub fn get_app_snapshot(state: State<'_, AppState>) -> AppResult<AppSnapshot> {
    state.snapshot(false)
}

#[tauri::command]
pub fn request_permissions(state: State<'_, AppState>) -> AppResult<AppSnapshot> {
    state.snapshot(true)
}

#[tauri::command]
pub fn complete_onboarding(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: OnboardingSettings,
) -> AppResult<AppSnapshot> {
    apply_autostart(&app, settings.launch_at_login)?;
    state.update_settings(&SettingsPatch {
        onboarding_completed: Some(true),
        launch_at_login: Some(settings.launch_at_login),
        ..Default::default()
    })?;
    state.snapshot(false)
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: SettingsPatch,
) -> AppResult<AppSnapshot> {
    if let Some(value) = settings.launch_at_login {
        apply_autostart(&app, value)?;
    }
    let updated = state.update_settings(&settings)?;
    let _ = app.emit("settings-changed", updated);
    state.snapshot(false)
}

#[tauri::command]
pub fn get_oakos_integration() -> AppResult<OakOsIntegration> {
    oakos::integration()
}

#[tauri::command]
pub async fn connect_oakos(token: String) -> AppResult<OakOsIntegration> {
    tauri::async_runtime::spawn_blocking(move || oakos::connect(&token))
        .await
        .map_err(|error| AppError::Other(format!("OakOS connection task failed: {error}")))?
}

#[tauri::command]
pub fn disconnect_oakos(state: State<'_, AppState>) -> AppResult<AppSnapshot> {
    oakos::disconnect()?;
    state.snapshot(false)
}

#[tauri::command]
pub async fn list_oakos_projects() -> AppResult<Vec<OakOsProject>> {
    tauri::async_runtime::spawn_blocking(oakos::list_projects)
        .await
        .map_err(|error| AppError::Other(format!("OakOS project task failed: {error}")))?
}

#[tauri::command]
pub async fn publish_recording_to_oakos(
    app: AppHandle,
    id: String,
    project_id: String,
) -> AppResult<OakOsPublishResult> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let recording = state.repository.recording(&id)?;
        if recording.deleted_at.is_some() {
            return Err(AppError::Other(
                "restore this recording before publishing it".into(),
            ));
        }
        let audio = state.decrypt_recording(&id)?;
        oakos::publish_recording(&project_id, &recording.id, &recording.title, audio)
    })
    .await
    .map_err(|error| AppError::Other(format!("OakOS publishing task failed: {error}")))?
}

#[tauri::command]
pub fn start_recording(
    app: AppHandle,
    state: State<'_, AppState>,
    input: StartRecordingInput,
) -> AppResult<RecordingSession> {
    state.start(app, input)
}

#[tauri::command]
pub fn pause_recording(app: AppHandle, state: State<'_, AppState>) -> AppResult<RecordingSession> {
    state.pause(&app)
}

#[tauri::command]
pub fn resume_recording(app: AppHandle, state: State<'_, AppState>) -> AppResult<RecordingSession> {
    state.resume(&app)
}

#[tauri::command]
pub fn add_highlight(app: AppHandle, state: State<'_, AppState>) -> AppResult<RecordingSession> {
    state.highlight(&app)
}

#[tauri::command]
pub fn stop_recording(app: AppHandle, state: State<'_, AppState>) -> AppResult<Recording> {
    state.stop(&app)
}

#[tauri::command]
pub fn list_recordings(
    state: State<'_, AppState>,
    include_deleted: bool,
) -> AppResult<Vec<Recording>> {
    state.repository.list_recordings(include_deleted)
}

#[tauri::command]
pub fn rename_recording(
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> AppResult<Recording> {
    state.repository.rename_recording(&id, &title)
}

#[tauri::command]
pub fn delete_recording(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.repository.set_deleted(&id, true)
}

#[tauri::command]
pub fn restore_recording(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.repository.set_deleted(&id, false)
}

#[tauri::command]
pub fn delete_recordings(state: State<'_, AppState>, ids: Vec<String>) -> AppResult<()> {
    state.repository.set_deleted_many(&ids, true)
}

#[tauri::command]
pub fn restore_recordings(state: State<'_, AppState>, ids: Vec<String>) -> AppResult<()> {
    state.repository.set_deleted_many(&ids, false)
}

#[tauri::command]
pub fn export_recording(state: State<'_, AppState>, id: String, path: String) -> AppResult<()> {
    let audio = state.decrypt_recording(&id)?;
    LocalExportDestination::export(Path::new(&path), &audio)
}

#[tauri::command]
pub fn get_recording_audio(state: State<'_, AppState>, id: String) -> AppResult<Vec<u8>> {
    state.decrypt_recording(&id)
}

#[tauri::command]
pub async fn transcribe_recording(app: AppHandle, id: String) -> AppResult<Recording> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AppState>().transcribe_recording(&app, &id)
    })
        .await
        .map_err(|error| AppError::Other(format!("transcription task failed: {error}")))?
}

#[tauri::command]
pub async fn summarize_recording(app: AppHandle, id: String) -> AppResult<Recording> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AppState>().summarize_recording(&app, &id)
    })
    .await
    .map_err(|error| AppError::Other(format!("summary task failed: {error}")))?
}

#[tauri::command]
pub fn list_summary_models(state: State<'_, AppState>) -> Vec<SummaryModelInfo> {
    state.summary_models()
}

#[tauri::command]
pub fn get_model_download_status(state: State<'_, AppState>) -> Option<ModelDownloadStatus> {
    state.model_download_status()
}

#[tauri::command]
pub async fn install_summary_model(app: AppHandle, model_id: String) -> AppResult<AppSnapshot> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AppState>().install_summary_model(&app, &model_id)
    })
    .await
    .map_err(|error| AppError::Other(format!("model installation task failed: {error}")))?
}

#[tauri::command]
pub fn remove_summary_model(
    state: State<'_, AppState>,
    model_id: String,
) -> AppResult<AppSnapshot> {
    state.remove_summary_model(&model_id)
}

#[tauri::command]
pub fn use_summary_model(
    state: State<'_, AppState>,
    model_id: String,
) -> AppResult<AppSnapshot> {
    state.use_summary_model(&model_id)
}

#[tauri::command]
pub fn list_whisper_models(state: State<'_, AppState>) -> Vec<WhisperModelInfo> {
    state.whisper_models()
}

#[tauri::command]
pub async fn install_whisper_model(app: AppHandle, model_id: String) -> AppResult<AppSnapshot> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AppState>()
            .install_whisper_model(&app, &model_id)
    })
    .await
    .map_err(|error| AppError::Other(format!("model installation task failed: {error}")))?
}

#[tauri::command]
pub fn remove_whisper_model(
    state: State<'_, AppState>,
    model_id: String,
) -> AppResult<AppSnapshot> {
    state.remove_whisper_model(&model_id)
}

#[tauri::command]
pub fn use_whisper_model(
    state: State<'_, AppState>,
    model_id: String,
) -> AppResult<AppSnapshot> {
    state.use_whisper_model(&model_id)
}

#[tauri::command]
pub fn open_library(app: AppHandle, recording_id: Option<String>) -> AppResult<()> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::Other("library window is unavailable".into()))?;
    window
        .show()
        .map_err(|error| AppError::Other(error.to_string()))?;
    window
        .set_focus()
        .map_err(|error| AppError::Other(error.to_string()))?;
    if let Some(id) = recording_id {
        let _ = window.emit("open-recording", id);
    }
    if let Some(quick) = app.get_webview_window("quick_panel") {
        let _ = quick.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn show_quick_panel(app: AppHandle) -> AppResult<()> {
    let quick = app
        .get_webview_window("quick_panel")
        .ok_or_else(|| AppError::Other("quick recorder window is unavailable".into()))?;

    if let Some(main) = app.get_webview_window("main")
        && let (Ok(position), Ok(main_size), Ok(quick_size)) =
            (main.outer_position(), main.outer_size(), quick.outer_size())
    {
        let scale = main.scale_factor().unwrap_or(1.0);
        let margin = (16.0 * scale).round() as i32;
        let top_offset = (44.0 * scale).round() as i32;
        let x = position.x + main_size.width as i32 - quick_size.width as i32 - margin;
        let y = position.y + top_offset;
        let _ = quick.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
            x, y,
        )));
    }

    quick
        .show()
        .map_err(|error| AppError::Other(error.to_string()))?;
    quick
        .set_focus()
        .map_err(|error| AppError::Other(error.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn hide_quick_panel(app: AppHandle) -> AppResult<()> {
    if let Some(window) = app.get_webview_window("quick_panel") {
        window
            .hide()
            .map_err(|error| AppError::Other(error.to_string()))?;
    }
    Ok(())
}

#[tauri::command]
pub fn resize_quick_panel(app: AppHandle, height: f64) -> AppResult<()> {
    let quick = app
        .get_webview_window("quick_panel")
        .ok_or_else(|| AppError::Other("quick recorder window is unavailable".into()))?;
    let height = normalized_quick_panel_height(height);
    let scale = quick
        .scale_factor()
        .map_err(|error| AppError::Other(error.to_string()))?;
    let width = quick
        .inner_size()
        .map_err(|error| AppError::Other(error.to_string()))?
        .width as f64
        / scale;
    quick
        .set_size(tauri::Size::Logical(tauri::LogicalSize::new(width, height)))
        .map_err(|error| AppError::Other(error.to_string()))
}

fn normalized_quick_panel_height(height: f64) -> f64 {
    if height.is_finite() {
        height.clamp(180.0, 520.0)
    } else {
        330.0
    }
}

#[tauri::command]
pub fn begin_update_install(state: State<'_, AppState>) -> AppResult<()> {
    state.begin_update_install()
}

#[tauri::command]
pub fn cancel_update_install(state: State<'_, AppState>) {
    state.cancel_update_install();
}

#[tauri::command]
pub fn dismiss_meeting(detector: State<'_, MeetingDetector>, id: String) -> AppResult<()> {
    detector.dismiss(id);
    Ok(())
}

#[tauri::command]
pub fn export_diagnostics(diagnostics: State<'_, Diagnostics>, path: String) -> AppResult<()> {
    diagnostics.export(Path::new(&path))
}

#[tauri::command]
pub fn open_screen_recording_settings() -> AppResult<()> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        .spawn()
        .map_err(|error| AppError::Other(error.to_string()))?;
    Ok(())
}

fn apply_autostart(app: &AppHandle, enabled: bool) -> AppResult<()> {
    if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    }
    .map_err(|error| AppError::Other(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::normalized_quick_panel_height;

    #[test]
    fn quick_panel_height_is_bounded_and_rejects_invalid_values() {
        assert_eq!(normalized_quick_panel_height(296.0), 296.0);
        assert_eq!(normalized_quick_panel_height(20.0), 180.0);
        assert_eq!(normalized_quick_panel_height(900.0), 520.0);
        assert_eq!(normalized_quick_panel_height(f64::NAN), 330.0);
    }
}
