use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMode {
    InPerson,
    Online,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordingPhase {
    Idle,
    Starting,
    Recording,
    Paused,
    Finalizing,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Highlight {
    pub id: String,
    pub offset_ms: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub text: String,
    pub language: Option<String>,
    pub created_at: String,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub size_bytes: u64,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperModelDownloadProgress {
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub id: String,
    pub title: String,
    pub mode: CaptureMode,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_ms: i64,
    pub playable_duration_ms: i64,
    pub status: String,
    pub size_bytes: i64,
    pub codec: String,
    pub detected_app: Option<String>,
    pub deleted_at: Option<String>,
    pub highlights: Vec<Highlight>,
    pub transcript: Option<Transcript>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingSession {
    pub phase: RecordingPhase,
    pub recording_id: Option<String>,
    pub mode: Option<CaptureMode>,
    pub started_at: Option<String>,
    pub elapsed_ms: i64,
    pub playable_ms: i64,
    pub mic_level: f32,
    pub system_level: f32,
    pub warning: Option<String>,
    pub error: Option<String>,
}

impl Default for RecordingSession {
    fn default() -> Self {
        Self {
            phase: RecordingPhase::Idle,
            recording_id: None,
            mode: None,
            started_at: None,
            elapsed_ms: 0,
            playable_ms: 0,
            mic_level: 0.0,
            system_level: 0.0,
            warning: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionValue {
    Granted,
    Denied,
    NotDetermined,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionState {
    pub microphone: PermissionValue,
    pub system_audio: PermissionValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub onboarding_completed: bool,
    pub meeting_detection_enabled: bool,
    pub launch_at_login: bool,
    pub microphone_id: Option<String>,
    pub whisper_model_path: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            onboarding_completed: false,
            meeting_detection_enabled: true,
            launch_at_login: false,
            microphone_id: None,
            whisper_model_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub session: RecordingSession,
    pub permissions: PermissionState,
    pub settings: AppSettings,
    pub devices: Vec<AudioDevice>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRecordingInput {
    pub mode: CaptureMode,
    pub detected_app: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub onboarding_completed: Option<bool>,
    pub meeting_detection_enabled: Option<bool>,
    pub launch_at_login: Option<bool>,
    pub microphone_id: Option<Option<String>>,
    pub whisper_model_path: Option<Option<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingSettings {
    pub launch_at_login: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioLevels {
    pub mic: f32,
    pub system: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingCandidate {
    pub id: String,
    pub app: String,
    pub display_name: String,
    pub detected_at: String,
}
