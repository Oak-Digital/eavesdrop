use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    process::Command,
    sync::mpsc::{self, Sender},
    thread,
    time::{Duration, Instant},
};

use chrono::Utc;
use tauri::{AppHandle, Emitter, Manager};

use crate::models::MeetingCandidate;
use crate::state::AppState;

pub struct MeetingDetector {
    dismiss_sender: Sender<String>,
}

impl MeetingDetector {
    pub fn start(app: AppHandle) -> Self {
        let (dismiss_sender, dismiss_receiver) = mpsc::channel::<String>();
        thread::Builder::new()
            .name("eavesdrop-meeting-detector".into())
            .spawn(move || {
                let mut first_seen: HashMap<String, Instant> = HashMap::new();
                let mut dismissed = HashSet::new();
                let mut emitted: Option<String> = None;
                loop {
                    while let Ok(id) = dismiss_receiver.try_recv() {
                        dismissed.insert(id);
                    }
                    let enabled = app
                        .state::<AppState>()
                        .repository
                        .settings()
                        .map(|settings| settings.meeting_detection_enabled)
                        .unwrap_or(false);
                    let candidates = if enabled {
                        platform_candidates()
                    } else {
                        Vec::new()
                    };
                    let present: HashSet<String> = candidates
                        .iter()
                        .map(|candidate| candidate.id.clone())
                        .collect();
                    first_seen.retain(|id, _| present.contains(id));
                    dismissed.retain(|id| present.contains(id));
                    if emitted.as_ref().is_some_and(|id| !present.contains(id)) {
                        emitted = None;
                        let _ = app.emit::<Option<MeetingCandidate>>("meeting-candidate", None);
                        let _ = app.emit("meeting-ended", ());
                    }
                    for candidate in candidates {
                        let seen_at = first_seen
                            .entry(candidate.id.clone())
                            .or_insert_with(Instant::now);
                        if seen_at.elapsed() >= Duration::from_secs(8)
                            && !dismissed.contains(&candidate.id)
                            && emitted.is_none()
                        {
                            emitted = Some(candidate.id.clone());
                            let _ = app.emit("meeting-candidate", &candidate);
                            if let Some(window) = app.get_webview_window("quick_panel") {
                                if let Ok(Some(monitor)) = window.primary_monitor() {
                                    let size = monitor.size();
                                    let scale = monitor.scale_factor();
                                    let x = size.width as f64 / scale - 380.0;
                                    let _ = window.set_position(tauri::Position::Logical(
                                        tauri::LogicalPosition::new(x, 36.0),
                                    ));
                                }
                                let _ = window.show();
                            }
                        }
                    }
                    thread::sleep(Duration::from_secs(4));
                }
            })
            .expect("meeting detector thread");
        Self { dismiss_sender }
    }

    pub fn dismiss(&self, id: String) {
        let _ = self.dismiss_sender.send(id);
    }
}

fn candidate(app: &str, display_name: &str) -> MeetingCandidate {
    let mut hasher = DefaultHasher::new();
    app.hash(&mut hasher);
    display_name.hash(&mut hasher);
    MeetingCandidate {
        id: format!("{app}-{:x}", hasher.finish()),
        app: app.into(),
        display_name: display_name.into(),
        detected_at: Utc::now().to_rfc3339(),
    }
}

fn match_candidates(processes: &str, windows: &str) -> Vec<MeetingCandidate> {
    let processes = processes.to_lowercase();
    let window_lines: Vec<&str> = windows
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let mut result = Vec::new();
    if processes.contains("zoom")
        && (processes.contains("cpthost")
            || window_lines
                .iter()
                .any(|title| title.to_lowercase().contains("zoom meeting")))
    {
        result.push(candidate("zoom", "Zoom"));
    }
    if processes.contains("teams")
        && window_lines.iter().any(|title| {
            let title = title.to_lowercase();
            title.contains("meeting") || title.contains("call")
        })
    {
        result.push(candidate("teams", "Microsoft Teams"));
    }
    if window_lines.iter().any(|title| {
        let title = title.to_lowercase();
        title.contains("google meet")
            || title.contains("meet - ")
            || title.contains("meet.google.com")
    }) {
        result.push(candidate("meet", "Google Meet"));
    }
    result
}

#[cfg(target_os = "macos")]
fn platform_candidates() -> Vec<MeetingCandidate> {
    let processes = Command::new("ps")
        .args(["-axo", "comm,args"])
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default();
    let script = r#"tell application "System Events" to get name of every window of every application process whose background only is false"#;
    let windows = Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).replace(", ", "\n"))
        .unwrap_or_default();
    match_candidates(&processes, &windows)
}

#[cfg(target_os = "windows")]
fn platform_candidates() -> Vec<MeetingCandidate> {
    let processes = Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default();
    let windows = Command::new("powershell").args(["-NoProfile", "-Command", "Get-Process | Where-Object {$_.MainWindowTitle} | Select-Object -ExpandProperty MainWindowTitle"]).output().ok().map(|out| String::from_utf8_lossy(&out.stdout).into_owned()).unwrap_or_default();
    match_candidates(&processes, &windows)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn platform_candidates() -> Vec<MeetingCandidate> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_meetings_without_generic_browser_false_positive() {
        assert_eq!(match_candidates("Google Chrome", "Inbox").len(), 0);
        assert_eq!(
            match_candidates("Google Chrome", "Daily sync - Google Meet")[0].app,
            "meet"
        );
        assert_eq!(
            match_candidates("zoom.us CptHost", "Zoom Meeting")[0].app,
            "zoom"
        );
        assert_eq!(
            match_candidates("ms-teams", "Design review | Meeting")[0].app,
            "teams"
        );
    }
}
