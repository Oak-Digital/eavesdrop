export type CaptureMode = "in_person" | "online";
export type ThemePreference = "light" | "dark" | "system";
export type RecordingPhase =
  | "idle"
  | "starting"
  | "recording"
  | "paused"
  | "finalizing"
  | "ready"
  | "failed";

export interface AudioDevice {
  id: string;
  name: string;
  isDefault: boolean;
}

export interface Highlight {
  id: string;
  offsetMs: number;
  createdAt: string;
}

export interface TranscriptSegment {
  startMs: number;
  endMs: number;
  text: string;
}

export interface Transcript {
  source?: TranscriptionSource | null;
  model?: string | null;
  text: string;
  language: string | null;
  createdAt: string;
  segments: TranscriptSegment[];
}

export interface Summary {
  markdown?: string | null;
  templateId?: string | null;
  suggestedTitle: string;
  overview: string;
  keyPoints: string[];
  decisions: string[];
  actionItems: string[];
  model: string;
  createdAt: string;
}

export interface SummaryModelInfo {
  id: string;
  name: string;
  description: string;
  sizeBytes: number;
  installed: boolean;
}

export interface SummaryModelDownloadProgress {
  modelId: string;
  downloadedBytes: number;
  totalBytes: number;
}

export type SummarizationStage = "loading" | "analyzing" | "writing";

export interface SummarizationProgress {
  recordingId: string;
  stage: SummarizationStage;
  progress: number;
}

export interface WhisperModelInfo {
  id: string;
  name: string;
  description: string;
  sizeBytes: number;
  installed: boolean;
}

export interface WhisperModelDownloadProgress {
  modelId: string;
  downloadedBytes: number;
  totalBytes: number;
}

export interface ModelDownloadStatus {
  kind: "whisper" | "summary";
  modelId: string;
  downloadedBytes: number;
  totalBytes: number;
  active: boolean;
}

export type TranscriptionStage = "decoding" | "transcribing" | "cloudTranscribing";

export interface TranscriptionProgress {
  recordingId: string;
  stage: TranscriptionStage;
  progress: number;
}

export interface Recording {
  id: string;
  title: string;
  mode: CaptureMode;
  startedAt: string;
  endedAt: string | null;
  durationMs: number;
  playableDurationMs: number;
  status: "recording" | "ready" | "recovered" | "failed";
  sizeBytes: number;
  codec: string;
  detectedApp: string | null;
  deletedAt: string | null;
  highlights: Highlight[];
  transcript: Transcript | null;
  summary: Summary | null;
}

export interface RecordingSession {
  phase: RecordingPhase;
  recordingId: string | null;
  mode: CaptureMode | null;
  startedAt: string | null;
  elapsedMs: number;
  playableMs: number;
  micLevel: number;
  systemLevel: number;
  warning: string | null;
  error: string | null;
}

export interface PermissionState {
  microphone: "granted" | "denied" | "not_determined" | "unavailable";
  systemAudio: "granted" | "denied" | "not_determined" | "unavailable";
}

export type TranscriptionSource = "local" | "cloud";
export interface TranscriptionTarget { source: TranscriptionSource }

export type SummarySource = "local" | "cloud";
export type CloudProvider = "openai" | "anthropic";
export type SummaryTarget = { source: "local" } | { source: "cloud"; provider: CloudProvider; model: string };
export interface CloudKeyStatus { provider: CloudProvider; keyStored: boolean }

export interface AppSettings {
  autoTranscribe: boolean;
  autoSummarize: boolean;
  transcriptionSource: TranscriptionSource;
  summarySource: SummarySource;
  cloudProvider: CloudProvider;
  cloudModel: string;
  summaryTemplate: string;
  onboardingCompleted: boolean;
  meetingDetectionEnabled: boolean;
  launchAtLogin: boolean;
  theme: ThemePreference;
  microphoneId: string | null;
  whisperModelPath: string | null;
  summaryModelPath: string | null;
  summaryPrompt: string | null;
}

export interface OakOsIntegration {
  connected: boolean;
}

export interface OakOsProject {
  id: string;
  name: string;
  identifier?: string | null;
  client?: { id: string; name: string } | null;
}

export interface OakOsPublishResult {
  location: string | null;
}

export interface AppSnapshot {
  session: RecordingSession;
  permissions: PermissionState;
  settings: AppSettings;
  devices: AudioDevice[];
}

export interface MeetingCandidate {
  id: string;
  app: "zoom" | "teams" | "meet";
  displayName: string;
  detectedAt: string;
}

export interface AudioLevels {
  mic: number;
  system: number;
}

export interface StartRecordingInput {
  mode: CaptureMode;
  detectedApp?: string;
}

export interface AutomaticProcessingStatus {
  recordingId: string;
  title: string;
  kind: "transcription" | "summary";
  active: boolean;
  error: string | null;
}
