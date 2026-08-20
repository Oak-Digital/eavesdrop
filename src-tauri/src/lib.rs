mod audio;
mod commands;
mod crypto;
mod destinations;
mod detection;
mod diagnostics;
mod download;
mod error;
mod models;
mod platform;
mod state;
mod storage;
mod summarization;
mod transcription;

use tauri::{
    Emitter, Manager,
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            commands::get_app_snapshot,
            commands::request_permissions,
            commands::complete_onboarding,
            commands::update_settings,
            commands::start_recording,
            commands::pause_recording,
            commands::resume_recording,
            commands::add_highlight,
            commands::stop_recording,
            commands::list_recordings,
            commands::rename_recording,
            commands::delete_recording,
            commands::restore_recording,
            commands::delete_recordings,
            commands::restore_recordings,
            commands::export_recording,
            commands::get_recording_audio,
            commands::transcribe_recording,
            commands::list_whisper_models,
            commands::install_whisper_model,
            commands::summarize_recording,
            commands::list_summary_models,
            commands::install_summary_model,
            commands::open_library,
            commands::hide_quick_panel,
            commands::begin_update_install,
            commands::cancel_update_install,
            commands::dismiss_meeting,
            commands::export_diagnostics,
            commands::open_screen_recording_settings,
        ])
        .setup(|app| {
            let state = state::AppState::open(app.handle())?;
            app.manage(state);
            let app_data = app.path().app_data_dir()?;
            app.manage(diagnostics::Diagnostics::open(&app_data)?);
            let detector = detection::MeetingDetector::start(app.handle().clone());
            app.manage(detector);
            let open_item = MenuItemBuilder::with_id("open", "Open Eavesdrop").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit Eavesdrop").build(app)?;
            let tray_menu = MenuBuilder::new(app)
                .item(&open_item)
                .separator()
                .item(&quit_item)
                .build()?;
            TrayIconBuilder::with_id("eavesdrop-tray")
                .icon(tray_image(false))
                .icon_as_template(false)
                .tooltip("Eavesdrop — ready to record")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => show_library(app),
                    "quit" => request_safe_quit(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        position,
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                        && let Some(window) = tray.app_handle().get_webview_window("quick_panel")
                    {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let scale = window.scale_factor().unwrap_or(1.0);
                            let x = position.x - 360.0 * scale;
                            let y = position.y + 8.0 * scale;
                            let _ = window.set_position(tauri::Position::Physical(
                                tauri::PhysicalPosition::new(x as i32, y as i32),
                            ));
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main"
                && let tauri::WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Eavesdrop");
}

fn show_library(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn request_safe_quit(app: &tauri::AppHandle) {
    if !app.state::<state::AppState>().is_recording() {
        app.exit(0);
        return;
    }

    let confirmed = app
        .dialog()
        .message("Eavesdrop is recording. Stop and save the recording before quitting?")
        .title("Recording in progress")
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Stop & Quit".into(),
            "Keep Recording".into(),
        ))
        .blocking_show();
    if confirmed {
        let app = app.clone();
        std::thread::spawn(move || {
            if let Err(error) = app.state::<state::AppState>().stop(&app) {
                let _ = app.emit(
                    "capture-warning",
                    format!("Could not finalize before quitting: {error}"),
                );
                show_library(&app);
                return;
            }
            app.exit(0);
        });
    }
}

fn tray_image(recording: bool) -> Image<'static> {
    let width = 18usize;
    let height = 18usize;
    let mut rgba = vec![0u8; width * height * 4];
    let heights = [6usize, 12, 16, 12, 6];
    let color = if recording {
        [180, 35, 24, 255]
    } else {
        [94, 86, 77, 255]
    };
    for (bar, bar_height) in heights.iter().enumerate() {
        let x = 3 + bar * 3;
        let start = (height - bar_height) / 2;
        for y in start..start + bar_height {
            for dx in 0..2 {
                let index = (y * width + x + dx) * 4;
                rgba[index..index + 4].copy_from_slice(&color);
            }
        }
    }
    Image::new_owned(rgba, width as u32, height as u32)
}

pub(crate) fn update_tray(app: &tauri::AppHandle, recording: bool) {
    if let Some(tray) = app.tray_by_id("eavesdrop-tray") {
        let _ = tray.set_icon(Some(tray_image(recording)));
        let _ = tray.set_tooltip(Some(if recording {
            "Eavesdrop — recording"
        } else {
            "Eavesdrop — ready to record"
        }));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn content_policy_allows_in_memory_audio_playback() {
        let config = include_str!("../tauri.conf.json");
        assert!(config.contains("media-src 'self' blob:"));
    }

    #[test]
    fn capability_allows_native_window_dragging() {
        let capability = include_str!("../capabilities/default.json");
        assert!(capability.contains("core:window:allow-start-dragging"));
    }

    #[test]
    fn bundle_includes_the_product_icon() {
        let config = include_str!("../tauri.conf.json");
        assert!(config.contains("icons/icon.icns"));
        assert!(config.contains("icons/icon.ico"));
    }

    #[test]
    fn transcription_progress_event_name_matches_the_frontend_listener() {
        // The backend emit and the webview listener are only coupled by this
        // string, so a rename on either side silently breaks the progress bar.
        let backend = include_str!("transcription.rs");
        let frontend = include_str!("../../src/api.ts");
        assert!(backend.contains("\"transcription-progress\""));
        assert!(frontend.contains("listen<TranscriptionProgress>(\"transcription-progress\""));
    }

    #[test]
    fn summary_event_names_match_the_frontend_listeners() {
        // Same coupling as the transcription progress event: a rename on either
        // side silently breaks the sidebar and the model download bar.
        let backend = include_str!("state.rs");
        let frontend = include_str!("../../src/api.ts");
        assert!(backend.contains("\"summarization-progress\""));
        assert!(backend.contains("\"summary-model-download-progress\""));
        assert!(frontend.contains("listen<SummarizationProgress>(\"summarization-progress\""));
        assert!(
            frontend
                .contains("listen<SummaryModelDownloadProgress>(\"summary-model-download-progress\"")
        );
    }

    #[test]
    fn local_signature_uses_a_stable_designated_requirement() {
        let script = include_str!("../../scripts/sign-macos-local.sh");
        assert!(script.contains("designated => identifier"));
        assert!(script.contains("com.eavesdrop.recorder"));
    }
}
