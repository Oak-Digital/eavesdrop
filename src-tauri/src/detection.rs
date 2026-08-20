use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::mpsc::{self, Sender},
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
use std::process::Command;

use chrono::Utc;
use tauri::{AppHandle, Emitter, Manager};

use crate::diagnostics::Diagnostics;
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
                let mut poll_failing = false;
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
                        match guarded_candidates(platform_candidates) {
                            Some(candidates) => {
                                poll_failing = false;
                                candidates
                            }
                            None => {
                                // Log the start of a failing streak only, so a
                                // persistent fault cannot flood the daily log
                                // with an entry every poll interval.
                                if !poll_failing {
                                    poll_failing = true;
                                    if let Some(diagnostics) = app.try_state::<Diagnostics>() {
                                        let _ = diagnostics.event("meeting_detection_poll_panicked");
                                    }
                                }
                                Vec::new()
                            }
                        }
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
                            // Show the already-loaded quick panel before publishing the
                            // candidate so a newly-created webview cannot miss the event.
                            let _ = app.emit("meeting-candidate", &candidate);
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

/// Runs one platform poll, containing any panic it raises.
///
/// The detector's `loop` lives entirely on the spawned thread, so an escaping
/// panic does not just drop one poll — it unwinds the thread and stops meeting
/// detection for the rest of the session with nothing surfaced to the user.
/// Returns `None` when the poll panicked, so the caller can record it.
fn guarded_candidates<F>(poll: F) -> Option<Vec<MeetingCandidate>>
where
    F: FnOnce() -> Vec<MeetingCandidate> + std::panic::UnwindSafe,
{
    std::panic::catch_unwind(poll).ok()
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct WindowInfo {
    owner: String,
    title: String,
}

impl WindowInfo {
    fn new(owner: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            owner: owner.into(),
            title: title.into(),
        }
    }
}

fn is_browser(value: &str) -> bool {
    ["chrome", "edge", "msedge", "firefox", "safari"]
        .iter()
        .any(|browser| value.contains(browser))
}

fn is_call_title(title: &str) -> bool {
    title.contains("meeting") || title.contains("call")
}

fn is_named_teams_meeting(title: &str) -> bool {
    (title.contains(" | microsoft teams") || title.contains(" - microsoft teams"))
        && title.trim() != "microsoft teams"
}

fn match_candidates(processes: &str, windows: &[WindowInfo]) -> Vec<MeetingCandidate> {
    let processes = processes.to_lowercase();
    let windows: Vec<(String, String)> = windows
        .iter()
        .map(|window| (window.owner.to_lowercase(), window.title.to_lowercase()))
        .collect();
    let mut result = Vec::new();
    if processes.contains("zoom")
        && (processes.contains("cpthost")
            || windows
                .iter()
                .any(|(_, title)| title.contains("zoom meeting")))
    {
        result.push(candidate("zoom", "Zoom"));
    }

    let native_teams_windows = windows
        .iter()
        .filter(|(owner, _)| owner.contains("teams"))
        .collect::<Vec<_>>();
    let native_teams_call = !native_teams_windows.is_empty()
        && (native_teams_windows.len() > 1
            || native_teams_windows
                .iter()
                .any(|(_, title)| is_call_title(title) || is_named_teams_meeting(title)));
    let browser_teams_call = windows.iter().any(|(owner, title)| {
        is_browser(owner)
            && (title.contains("teams.microsoft.com")
                || is_named_teams_meeting(title)
                || (title.contains("microsoft teams") && is_call_title(title)))
    });
    if (processes.contains("teams") && native_teams_call) || browser_teams_call {
        result.push(candidate("teams", "Microsoft Teams"));
    }

    if windows.iter().any(|(_, title)| {
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
    use screencapturekit::prelude::*;

    let Ok(content) = SCShareableContent::get() else {
        return Vec::new();
    };
    // Deliberately avoids `SCShareableContent::snapshot()`. Its batched collectors
    // in screencapturekit 8.0.1 index a `Vec::with_capacity(..)` scratch buffer that
    // still has length 0, so they panic as soon as the bridge reports a single
    // display, window, or application. The per-element accessors used below build
    // their vectors with `extend` and are sound. Detection polls every 4s, so the
    // extra FFI round-trips (~70us vs ~5us) do not matter here.
    let processes = content
        .applications()
        .iter()
        .flat_map(|application| [application.application_name(), application.bundle_identifier()])
        .collect::<Vec<_>>()
        .join("\n");
    let windows = content
        .windows()
        .iter()
        .filter(|window| {
            window.window_layer() == 0 && (window.is_on_screen() || window.is_active())
        })
        .filter_map(|window| {
            let title = window.title()?;
            let title = title.trim();
            if title.is_empty() {
                return None;
            }
            let owner = window
                .owning_application()
                .map(|application| application.application_name())
                .unwrap_or_default();
            Some(WindowInfo::new(owner, title))
        })
        .collect::<Vec<_>>();
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
    let windows = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-Process | Where-Object {$_.MainWindowTitle} | ForEach-Object { \"$($_.ProcessName)`t$($_.MainWindowTitle)\" }",
        ])
        .output()
        .ok()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter_map(|line| {
                    let (owner, title) = line.split_once('\t')?;
                    Some(WindowInfo::new(owner, title))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
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
    fn a_panicking_poll_is_contained_so_the_detector_loop_survives() {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let outcome = guarded_candidates(|| panic!("index out of bounds: the len is 0"));
        std::panic::set_hook(previous);

        assert!(outcome.is_none());
        // The poll after a panicking one still runs: the panic was contained
        // rather than unwinding the thread that owns the detection loop.
        let recovered = guarded_candidates(|| vec![candidate("zoom", "Zoom")]).unwrap();
        assert_eq!(
            recovered.iter().map(|c| c.app.as_str()).collect::<Vec<_>>(),
            vec!["zoom"]
        );
    }

    /// Guards the screencapturekit 8.0.1 regression: the batched `snapshot()`
    /// path panicked with "index out of bounds: the len is 0" as soon as the
    /// bridge reported any window, killing the detector thread. Returning an
    /// empty vec is a valid outcome here (no meeting running, or no screen
    /// recording permission in the test environment) — panicking is not.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_real_platform_poll_never_panics() {
        assert!(guarded_candidates(platform_candidates).is_some());
    }

    #[test]
    fn a_successful_poll_passes_its_candidates_through_unchanged() {
        let polled =
            guarded_candidates(|| vec![candidate("teams", "Microsoft Teams")]).expect("no panic");
        assert_eq!(polled.len(), 1);
        assert_eq!(polled[0].app, "teams");
        assert_eq!(polled[0].display_name, "Microsoft Teams");
        assert_eq!(polled[0].id, candidate("teams", "Microsoft Teams").id);
    }

    #[test]
    fn detects_supported_meetings_without_generic_browser_false_positive() {
        assert_eq!(
            match_candidates(
                "Google Chrome",
                &[WindowInfo::new("Google Chrome", "Inbox")]
            )
            .len(),
            0
        );
        assert_eq!(
            match_candidates(
                "Google Chrome",
                &[WindowInfo::new("Google Chrome", "Daily sync - Google Meet")]
            )[0]
            .app,
            "meet"
        );
        assert_eq!(
            match_candidates(
                "zoom.us CptHost",
                &[WindowInfo::new("zoom.us", "Zoom Meeting")]
            )[0]
            .app,
            "zoom"
        );
        assert_eq!(
            match_candidates(
                "ms-teams",
                &[WindowInfo::new(
                    "Microsoft Teams",
                    "Design review | Meeting"
                )]
            )[0]
            .app,
            "teams"
        );
    }

    #[test]
    fn detects_teams_in_supported_browsers() {
        for browser in ["Google Chrome", "Microsoft Edge", "Firefox", "Safari"] {
            let matches = match_candidates(
                browser,
                &[WindowInfo::new(browser, "Daily sync | Microsoft Teams")],
            );
            assert_eq!(
                matches.first().map(|candidate| candidate.app.as_str()),
                Some("teams")
            );
        }
    }

    #[test]
    fn native_teams_requires_a_call_window() {
        assert!(
            match_candidates(
                "ms-teams",
                &[WindowInfo::new("Microsoft Teams", "Microsoft Teams")]
            )
            .is_empty()
        );
        assert_eq!(
            match_candidates(
                "ms-teams",
                &[
                    WindowInfo::new("Microsoft Teams", "Microsoft Teams"),
                    WindowInfo::new("Microsoft Teams", "Weekly planning"),
                ]
            )[0]
            .app,
            "teams"
        );
    }
}
