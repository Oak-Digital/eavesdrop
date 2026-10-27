import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { confirm, open, save } from "@tauri-apps/plugin-dialog";
import type {
  AppSettings,
  AutomaticProcessingStatus,
  CloudKeyStatus,
  CloudProvider,
  SummaryTarget,
  AppSnapshot,
  AudioLevels,
  CaptureMode,
  MeetingCandidate,
  ModelDownloadStatus,
  OakOsIntegration,
  OakOsProject,
  OakOsPublishResult,
  Recording,
  RecordingSession,
  StartRecordingInput,
  SummarizationProgress,
  SummaryModelDownloadProgress,
  SummaryModelInfo,
  TranscriptionProgress,
  TranscriptionTarget,
  WhisperModelDownloadProgress,
  WhisperModelInfo,
} from "./types";

import { DEFAULT_SUMMARY_TEMPLATE, summaryTemplate } from "./summaryTemplates";

const isTauri = () => "__TAURI_INTERNALS__" in window;

const idleSession: RecordingSession = {
  phase: "idle",
  recordingId: null,
  mode: null,
  startedAt: null,
  elapsedMs: 0,
  playableMs: 0,
  micLevel: 0,
  systemLevel: 0,
  warning: null,
  error: null,
};

const mockSnapshot: AppSnapshot = {
  session: idleSession,
  permissions: { microphone: "not_determined", systemAudio: "not_determined" },
  settings: {
    onboardingCompleted: false,
    meetingDetectionEnabled: true,
    launchAtLogin: false,
    theme: "light",
    microphoneId: null,
    whisperModelPath: null,
    summaryModelPath: null,
    summaryPrompt: null,
    summaryTemplate: DEFAULT_SUMMARY_TEMPLATE,
    summarySource: "local",
    transcriptionSource: "local",
    autoTranscribe: false,
    autoSummarize: false,
    cloudProvider: "openai",
    cloudModel: "gpt-5.4-mini",
  },
  devices: [{ id: "default", name: "Default microphone", isDefault: true }],
};

let browserSnapshot = structuredClone(mockSnapshot);
let browserRecordings: Recording[] = [];
let browserInstalledWhisperModels = new Set<string>();
let browserInstalledSummaryModels = new Set<string>();
let browserRecordingClock: { startedAtMs: number; pausedAtMs: number | null; pausedTotalMs: number } | null = null;
let browserHighlights: Recording["highlights"] = [];
let browserOakOsConnected = false;
let browserCloudKeys = new Set<CloudProvider>();
const browserOakOsProjects: OakOsProject[] = [
  { id: "oak-project-one", name: "Product", identifier: "OAK2601", client: { id: "oak-client", name: "Oak Digital" } },
  { id: "oak-project-two", name: "Research", identifier: "DIF2602", client: { id: "dif-client", name: "DIF" } },
  { id: "oak-project-three", name: "Website", identifier: "OAK2603", client: { id: "oak-client", name: "Oak Digital" } },
];
const browserSessionListeners = new Set<(session: RecordingSession) => void>();
const browserRecordingFinalizedListeners = new Set<(recording: Recording) => void>();
const browserRecordingUpdatedListeners = new Set<(recording: Recording) => void>();
const browserAutomaticListeners = new Set<(status: AutomaticProcessingStatus) => void>();
let browserAutomaticStatus: AutomaticProcessingStatus | null = null;
let browserAutomaticQueue = Promise.resolve();
let browserGeneration = 0;

function emitBrowserRecordingUpdated(recording: Recording) {
  browserRecordingUpdatedListeners.forEach((handler) => handler(structuredClone(recording)));
}

function enqueueBrowserAutomatic(id: string, transcriptVersion?: string) {
  const generation = browserGeneration;
  browserAutomaticQueue = browserAutomaticQueue.then(async () => {
    if (generation !== browserGeneration) return;
    const settings = browserSnapshot.settings;
    const recording = browserRecordings.find((item) => item.id === id);
    if (!recording) return;
    const summary = transcriptVersion !== undefined;
    if (summary ? !settings.autoSummarize || recording.transcript?.createdAt !== transcriptVersion || !recording.transcript.text.trim()
      : !settings.autoTranscribe || recording.transcript !== null) return;
    const status: AutomaticProcessingStatus = { recordingId: id, title: recording.title, kind: summary ? "summary" : "transcription", active: true, error: null };
    const publish = () => {
      if (generation !== browserGeneration) return;
      browserAutomaticStatus = structuredClone(status);
      browserAutomaticListeners.forEach((handler) => handler(structuredClone(status)));
    };
    publish();
    try {
      if (summary) {
        if (settings.summarySource === "local" && !settings.summaryModelPath) throw new Error("Install a Local summary model in Settings first");
        emitBrowserRecordingUpdated(await summarizeRecording(id));
      } else {
        if (settings.transcriptionSource === "local" && !settings.whisperModelPath) throw new Error("Choose a local Whisper model in Settings first");
        emitBrowserRecordingUpdated(await transcribeRecording(id));
      }
    } catch (cause) { status.error = cause instanceof Error ? cause.message : String(cause); }
    finally { status.active = false; publish(); }
  });
}


function updateBrowserSessionTiming(now = Date.now()) {
  if (!browserRecordingClock) return;
  const elapsedMs = Math.max(0, now - browserRecordingClock.startedAtMs);
  const currentPauseMs = browserRecordingClock.pausedAtMs === null ? 0 : Math.max(0, now - browserRecordingClock.pausedAtMs);
  browserSnapshot.session.elapsedMs = elapsedMs;
  browserSnapshot.session.playableMs = Math.max(0, elapsedMs - browserRecordingClock.pausedTotalMs - currentPauseMs);
}

function emitBrowserSession() {
  const session = structuredClone(browserSnapshot.session);
  browserSessionListeners.forEach((handler) => handler(structuredClone(session)));
}

function emitBrowserRecordingFinalized(recording: Recording) {
  browserRecordingFinalizedListeners.forEach((handler) => handler(structuredClone(recording)));
}

async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) throw new Error(`Command ${name} is only available in the desktop app`);
  return invoke<T>(name, args);
}

export async function getSnapshot(): Promise<AppSnapshot> {
  if (isTauri()) return command<AppSnapshot>("get_app_snapshot");
  updateBrowserSessionTiming();
  return structuredClone(browserSnapshot);
}

export async function requestPermissions(): Promise<AppSnapshot> {
  if (isTauri()) return command<AppSnapshot>("request_permissions");
  browserSnapshot.permissions = { microphone: "granted", systemAudio: "granted" };
  return structuredClone(browserSnapshot);
}

export async function completeOnboarding(settings: Pick<AppSettings, "launchAtLogin">): Promise<AppSnapshot> {
  if (isTauri()) return command<AppSnapshot>("complete_onboarding", { settings });
  browserSnapshot.settings = { ...browserSnapshot.settings, ...settings, onboardingCompleted: true };
  return structuredClone(browserSnapshot);
}

export async function updateSettings(settings: Partial<AppSettings>): Promise<AppSnapshot> {
  if (isTauri()) return command<AppSnapshot>("update_settings", { settings });
  browserSnapshot.settings = { ...browserSnapshot.settings, ...settings };
  return structuredClone(browserSnapshot);
}

export async function getOakOsIntegration(): Promise<OakOsIntegration> {
  if (isTauri()) return command<OakOsIntegration>("get_oakos_integration");
  return { connected: browserOakOsConnected };
}

export async function connectOakOs(token: string): Promise<OakOsIntegration> {
  if (isTauri()) return command<OakOsIntegration>("connect_oakos", { token });
  if (!token.trim()) throw new Error("enter an OakOS personal access token");
  browserOakOsConnected = true;
  return { connected: true };
}

export async function disconnectOakOs(): Promise<AppSnapshot> {
  if (isTauri()) return command<AppSnapshot>("disconnect_oakos");
  browserOakOsConnected = false;
  return structuredClone(browserSnapshot);
}

export async function listOakOsProjects(): Promise<OakOsProject[]> {
  if (isTauri()) return command<OakOsProject[]>("list_oakos_projects");
  if (!browserOakOsConnected) throw new Error("OakOS is not connected");
  return structuredClone(browserOakOsProjects);
}

export async function publishRecordingToOakOs(id: string, projectId: string): Promise<OakOsPublishResult> {
  if (isTauri()) return command<OakOsPublishResult>("publish_recording_to_oakos", { id, projectId });
  if (!browserOakOsConnected) throw new Error("OakOS is not connected");
  if (!browserRecordings.some((recording) => recording.id === id)) throw new Error("recording not found");
  if (!browserOakOsProjects.some((project) => project.id === projectId)) throw new Error("OakOS project not found");
  return { location: `/api/v1/recordings/${id}` };
}

export async function startRecording(input: StartRecordingInput): Promise<RecordingSession> {
  if (isTauri()) return command<RecordingSession>("start_recording", { input });
  if (["starting", "recording", "paused", "finalizing"].includes(browserSnapshot.session.phase)) {
    throw new Error("a recording is already active");
  }
  const startedAtMs = Date.now();
  browserRecordingClock = { startedAtMs, pausedAtMs: null, pausedTotalMs: 0 };
  browserHighlights = [];
  browserSnapshot.session = {
    ...idleSession,
    phase: "recording",
    recordingId: crypto.randomUUID(),
    mode: input.mode,
    startedAt: new Date(startedAtMs).toISOString(),
  };
  emitBrowserSession();
  return structuredClone(browserSnapshot.session);
}

export async function pauseRecording(): Promise<RecordingSession> {
  if (isTauri()) return command<RecordingSession>("pause_recording");
  if (browserSnapshot.session.phase !== "recording" || !browserRecordingClock) {
    throw new Error("recording is not active");
  }
  const now = Date.now();
  updateBrowserSessionTiming(now);
  browserRecordingClock.pausedAtMs = now;
  browserSnapshot.session.phase = "paused";
  emitBrowserSession();
  return structuredClone(browserSnapshot.session);
}

export async function resumeRecording(): Promise<RecordingSession> {
  if (isTauri()) return command<RecordingSession>("resume_recording");
  if (browserSnapshot.session.phase !== "paused" || !browserRecordingClock || browserRecordingClock.pausedAtMs === null) {
    throw new Error("recording is not paused");
  }
  const now = Date.now();
  browserRecordingClock.pausedTotalMs += Math.max(0, now - browserRecordingClock.pausedAtMs);
  browserRecordingClock.pausedAtMs = null;
  browserSnapshot.session.phase = "recording";
  updateBrowserSessionTiming(now);
  emitBrowserSession();
  return structuredClone(browserSnapshot.session);
}

export async function setRecordingMode(mode: CaptureMode): Promise<RecordingSession> {
  if (isTauri()) return command<RecordingSession>("set_recording_mode", { mode });
  if (!["recording", "paused"].includes(browserSnapshot.session.phase)) {
    throw new Error("sources can only change during an active recording");
  }
  updateBrowserSessionTiming();
  browserSnapshot.session.mode = mode;
  browserSnapshot.session.systemLevel = 0;
  emitBrowserSession();
  return structuredClone(browserSnapshot.session);
}

export async function addHighlight(): Promise<RecordingSession> {
  if (isTauri()) return command<RecordingSession>("add_highlight");
  if (browserSnapshot.session.phase !== "recording") throw new Error("resume before adding a highlight");
  updateBrowserSessionTiming();
  browserHighlights.push({
    id: crypto.randomUUID(),
    offsetMs: browserSnapshot.session.playableMs,
    createdAt: new Date().toISOString(),
  });
  emitBrowserSession();
  return structuredClone(browserSnapshot.session);
}

export async function stopRecording(): Promise<Recording> {
  if (isTauri()) return command<Recording>("stop_recording");
  if (!["recording", "paused"].includes(browserSnapshot.session.phase) || !browserRecordingClock) {
    throw new Error("no recording is active");
  }
  const stoppedAtMs = Date.now();
  updateBrowserSessionTiming(stoppedAtMs);
  browserSnapshot.session.phase = "finalizing";
  emitBrowserSession();
  const recording: Recording = {
    id: browserSnapshot.session.recordingId!,
    title: `${browserSnapshot.session.mode === "online" ? "Online" : "In-person"} meeting`,
    mode: browserSnapshot.session.mode!,
    startedAt: browserSnapshot.session.startedAt!,
    endedAt: new Date(stoppedAtMs).toISOString(),
    durationMs: browserSnapshot.session.elapsedMs,
    playableDurationMs: browserSnapshot.session.playableMs,
    status: "ready",
    sizeBytes: 0,
    codec: "AAC-LC",
    detectedApp: null,
    deletedAt: null,
    highlights: structuredClone(browserHighlights),
    transcript: null,
    summary: null,
  };
  browserRecordings.unshift(recording);
  browserSnapshot.session = structuredClone(idleSession);
  browserRecordingClock = null;
  browserHighlights = [];
  emitBrowserSession();
  emitBrowserRecordingFinalized(recording);
  if (browserSnapshot.settings.autoTranscribe) enqueueBrowserAutomatic(recording.id);
  return structuredClone(recording);
}

export async function listRecordings(): Promise<Recording[]> {
  if (isTauri()) return command<Recording[]>("list_recordings");
  return structuredClone(browserRecordings);
}

export async function renameRecording(id: string, title: string): Promise<Recording> {
  if (isTauri()) return command<Recording>("rename_recording", { id, title });
  const item = browserRecordings.find((recording) => recording.id === id)!;
  item.title = title;
  return structuredClone(item);
}

export async function confirmRecordingDeletion(recordings: Pick<Recording, "title">[]): Promise<boolean> {
  if (!recordings.length) return false;
  const subject = recordings.length === 1 ? `“${recordings[0].title}”` : `${recordings.length} recordings`;
  const message = `Permanently delete ${subject}? The audio, transcripts, summaries, and highlights will be removed from this computer. This cannot be undone.`;
  if (!isTauri()) return window.confirm(message);
  return confirm(message, {
    title: recordings.length === 1 ? "Delete recording?" : "Delete recordings?",
    kind: "warning",
    okLabel: "Delete permanently",
    cancelLabel: "Cancel",
  });
}

export async function deleteRecording(id: string): Promise<void> {
  if (isTauri()) return command<void>("delete_recording", { id });
  return deleteRecordings([id]);
}

export async function deleteRecordings(ids: string[]): Promise<void> {
  if (isTauri()) return command<void>("delete_recordings", { ids });
  if (ids.some((id) => !browserRecordings.some((item) => item.id === id))) throw new Error("Recording not found");
  browserRecordings = browserRecordings.filter((item) => !ids.includes(item.id));
}

export async function exportRecording(recording: Recording): Promise<void> {
  if (!isTauri()) return;
  const path = await save({
    defaultPath: `${recording.title.replace(/[\\/:*?"<>|]/g, "-")}.m4a`,
    filters: [{ name: "MPEG-4 audio", extensions: ["m4a"] }],
  });
  if (path) await command<void>("export_recording", { id: recording.id, path });
}

export async function exportDiagnostics(): Promise<void> {
  if (!isTauri()) return;
  const path = await save({
    defaultPath: "Eavesdrop diagnostics.log",
    filters: [{ name: "Log file", extensions: ["log"] }],
  });
  if (path) await command<void>("export_diagnostics", { path });
}

export async function openScreenRecordingSettings(): Promise<void> {
  if (isTauri()) await command<void>("open_screen_recording_settings");
}

export async function getRecordingAudio(id: string): Promise<Uint8Array | null> {
  if (!isTauri()) return null;
  const bytes = await command<number[]>("get_recording_audio", { id });
  return Uint8Array.from(bytes);
}

export async function selectWhisperModel(): Promise<string | null> {
  if (!isTauri()) return "/models/ggml-base.bin";
  const path = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Whisper model", extensions: ["bin"] }],
  });
  return typeof path === "string" ? path : null;
}

export async function transcribeRecording(id: string, target?: TranscriptionTarget): Promise<Recording> {
  if (isTauri()) return command<Recording>("transcribe_recording", { id, target });
  const source = target?.source ?? browserSnapshot.settings.transcriptionSource;
  if (source === "cloud" && !browserCloudKeys.has("openai")) throw new Error("Add your OpenAI API key in Settings first");
  const item = browserRecordings.find((recording) => recording.id === id);
  if (!item) throw new Error("Recording not found");
  const text = source === "cloud" ? "Example cloud transcript." : "Example local transcript.";
  item.transcript = {
    text,
    language: "English",
    createdAt: new Date().toISOString(),
    segments: source === "cloud" ? [] : [{ startMs: 0, endMs: 2000, text }],
    source,
    model: source === "cloud" ? "gpt-transcribe" : "Whisper",
  };
  if (browserSnapshot.settings.autoSummarize) enqueueBrowserAutomatic(id, item.transcript.createdAt);
  return structuredClone(item);
}

export async function selectSummaryModel(): Promise<string | null> {
  if (!isTauri()) return "/models/summary.gguf";
  const path = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "GGUF model", extensions: ["gguf"] }],
  });
  return typeof path === "string" ? path : null;
}

export async function summarizeRecording(id: string, templateId?: string, target?: SummaryTarget): Promise<Recording> {
  if (isTauri()) return command<Recording>("summarize_recording", { id, templateId, target });
  const settings = browserSnapshot.settings;
  const selectedTarget: SummaryTarget = target ?? (settings.summarySource === "local" ? { source: "local" } : { source: "cloud", provider: settings.cloudProvider, model: settings.cloudModel });
  if (selectedTarget.source === "cloud" && !browserCloudKeys.has(selectedTarget.provider)) throw new Error("Add your Cloud API key in Settings first");
  const selectedId = templateId ?? browserSnapshot.settings.summaryTemplate;
  const template = summaryTemplate(selectedId);
  if (!template) throw new Error(`Unknown summary template: ${selectedId}`);
  const item = browserRecordings.find((recording) => recording.id === id)!;
  item.summary = {
    templateId: selectedId,
    markdown: template.markdown ? (selectedId === "discussion"
      ? "# Example local summary\n\n## 1. Planning\n\n### Conclusion\nThe team discussed the next release.\n\n### Discussion points\n- The scope needs clarification.\n\n## 2. Follow-up\n\n### Agreed actions\n- [ ] Clarify the scope. Owner and deadline not specified.\n\n### Proposed follow-up\nNone.\n\n## Unresolved matters\nThe release date remains open."
      : "# Example local summary\n\n## Agreed actions\n- [ ] Clarify the scope. Owner and deadline not specified.\n\n## Proposed follow-up\nNone.\n\n## Open decisions\nThe release date remains open.") : null,
    suggestedTitle: selectedTarget.source === "cloud" ? "Example Cloud summary" : "Example local summary",
    overview: selectedTarget.source === "cloud" ? "A simulated Cloud summary of the meeting." : "A short, on-device summary of the meeting.",
    keyPoints: ["The first thing discussed."],
    decisions: ["Ship it."],
    actionItems: ["Someone follows up."],
    model: selectedTarget.source === "cloud" ? `Cloud · ${selectedTarget.provider === "openai" ? "OpenAI" : "Anthropic"} / ${selectedTarget.model}` : "summary.gguf",
    createdAt: new Date().toISOString(),
  };
  if (selectedTarget.source === "cloud" && item.summary.markdown) item.summary.markdown = item.summary.markdown.replace("Example local summary", "Example Cloud summary");
  item.title = item.summary.suggestedTitle;
  return structuredClone(item);
}

export async function listSummaryModels(): Promise<SummaryModelInfo[]> {
  if (isTauri()) return command<SummaryModelInfo[]>("list_summary_models");
  return [
    { id: "qwen2.5-1.5b", name: "Compact", description: "Fast on any Mac", sizeBytes: 1_117_320_736 },
    { id: "qwen2.5-7b", name: "Detailed", description: "Sharper, slower", sizeBytes: 4_683_074_240 },
  ].map((model) => ({ ...model, installed: browserInstalledSummaryModels.has(model.id) }));
}

export async function installSummaryModel(modelId: string): Promise<AppSnapshot> {
  if (isTauri()) return command<AppSnapshot>("install_summary_model", { modelId });
  browserInstalledSummaryModels.add(modelId);
  browserSnapshot.settings.summaryModelPath = `/models/${modelId}.gguf`;
  browserSnapshot.settings.summarySource = "local";
  return structuredClone(browserSnapshot);
}

export async function useSummaryModel(modelId: string): Promise<AppSnapshot> {
  if (isTauri()) return command<AppSnapshot>("use_summary_model", { modelId });
  if (!browserInstalledSummaryModels.has(modelId)) throw new Error("This summary model is not installed");
  browserSnapshot.settings.summaryModelPath = `/models/${modelId}.gguf`;
  browserSnapshot.settings.summarySource = "local";
  return structuredClone(browserSnapshot);
}

export async function removeSummaryModel(modelId: string): Promise<AppSnapshot> {
  if (isTauri()) return command<AppSnapshot>("remove_summary_model", { modelId });
  browserInstalledSummaryModels.delete(modelId);
  if (browserSnapshot.settings.summaryModelPath === `/models/${modelId}.gguf`) {
    browserSnapshot.settings.summaryModelPath = null;
  }
  return structuredClone(browserSnapshot);
}

export async function onSummarizationProgress(handler: (progress: SummarizationProgress) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<SummarizationProgress>("summarization-progress", (event) => handler(event.payload));
}

export async function onSummaryModelDownloadProgress(handler: (progress: SummaryModelDownloadProgress) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<SummaryModelDownloadProgress>("summary-model-download-progress", (event) => handler(event.payload));
}

export async function listWhisperModels(): Promise<WhisperModelInfo[]> {
  if (isTauri()) return command<WhisperModelInfo[]>("list_whisper_models");
  return [
    { id: "tiny", name: "Tiny", description: "Fastest, with lower accuracy", sizeBytes: 77_691_713 },
    { id: "base", name: "Base", description: "Recommended balance of speed and accuracy", sizeBytes: 147_951_465 },
    { id: "small", name: "Small", description: "More accurate, but slower", sizeBytes: 487_601_967 },
    { id: "large-v3-turbo-q5_0", name: "Turbo", description: "Most accurate, and fast on Apple silicon", sizeBytes: 574_041_195 },
  ].map((model) => ({ ...model, installed: browserInstalledWhisperModels.has(model.id) }));
}

export async function installWhisperModel(modelId: string): Promise<AppSnapshot> {
  if (isTauri()) return command<AppSnapshot>("install_whisper_model", { modelId });
  browserInstalledWhisperModels.add(modelId);
  browserSnapshot.settings.transcriptionSource = "local";
  browserSnapshot.settings.whisperModelPath = `/models/ggml-${modelId}.bin`;
  return structuredClone(browserSnapshot);
}

export async function useWhisperModel(modelId: string): Promise<AppSnapshot> {
  if (isTauri()) return command<AppSnapshot>("use_whisper_model", { modelId });
  if (!browserInstalledWhisperModels.has(modelId)) throw new Error("This Whisper model is not installed");
  browserSnapshot.settings.transcriptionSource = "local";
  browserSnapshot.settings.whisperModelPath = `/models/ggml-${modelId}.bin`;
  return structuredClone(browserSnapshot);
}

export async function removeWhisperModel(modelId: string): Promise<AppSnapshot> {
  if (isTauri()) return command<AppSnapshot>("remove_whisper_model", { modelId });
  browserInstalledWhisperModels.delete(modelId);
  if (browserSnapshot.settings.whisperModelPath === `/models/ggml-${modelId}.bin`) {
    browserSnapshot.settings.whisperModelPath = null;
  }
  return structuredClone(browserSnapshot);
}

export async function onTranscriptionProgress(handler: (progress: TranscriptionProgress) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<TranscriptionProgress>("transcription-progress", (event) => handler(event.payload));
}

export async function onWhisperModelDownloadProgress(handler: (progress: WhisperModelDownloadProgress) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<WhisperModelDownloadProgress>("whisper-model-download-progress", (event) => handler(event.payload));
}

export async function getModelDownloadStatus(): Promise<ModelDownloadStatus | null> {
  if (!isTauri()) return null;
  return command<ModelDownloadStatus | null>("get_model_download_status");
}

export async function onModelDownloadStatus(handler: (status: ModelDownloadStatus) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<ModelDownloadStatus>("model-download-status", (event) => handler(event.payload));
}

export async function dismissMeeting(id: string): Promise<void> {
  if (isTauri()) await command<void>("dismiss_meeting", { id });
}

export async function openLibrary(recordingId?: string): Promise<void> {
  if (isTauri()) await command<void>("open_library", { recordingId });
}

export async function showQuickPanel(): Promise<void> {
  if (isTauri()) await command<void>("show_quick_panel");
}

export async function hideQuickPanel(): Promise<void> {
  if (isTauri()) await command<void>("hide_quick_panel");
}

export async function resizeQuickPanel(height: number): Promise<void> {
  if (isTauri()) await command<void>("resize_quick_panel", { height });
}

export async function beginUpdateInstall(): Promise<void> {
  if (isTauri()) await command<void>("begin_update_install");
}

export async function cancelUpdateInstall(): Promise<void> {
  if (isTauri()) await command<void>("cancel_update_install");
}

export async function onSessionChanged(handler: (session: RecordingSession) => void): Promise<UnlistenFn> {
  if (!isTauri()) {
    browserSessionListeners.add(handler);
    return () => browserSessionListeners.delete(handler);
  }
  return listen<RecordingSession>("recording-state-changed", (event) => handler(event.payload));
}

export async function onSettingsChanged(handler: (settings: AppSettings) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<AppSettings>("settings-changed", (event) => handler(event.payload));
}

export async function onAudioLevels(handler: (levels: AudioLevels) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<AudioLevels>("audio-levels", (event) => handler(event.payload));
}

export async function onRecordingFinalized(handler: (recording: Recording) => void): Promise<UnlistenFn> {
  if (!isTauri()) {
    browserRecordingFinalizedListeners.add(handler);
    return () => browserRecordingFinalizedListeners.delete(handler);
  }
  return listen<Recording>("recording-finalized", (event) => handler(event.payload));
}

export async function onOpenRecording(handler: (recordingId: string) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<string>("open-recording", (event) => handler(event.payload));
}

export async function onMeetingCandidate(handler: (meeting: MeetingCandidate | null) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<MeetingCandidate | null>("meeting-candidate", (event) => handler(event.payload));
}

export async function onCaptureWarning(handler: (message: string) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<string>("capture-warning", (event) => handler(event.payload));
}

export async function onMeetingEnded(handler: () => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen("meeting-ended", handler);
}

export function resetBrowserMock() {
  browserSnapshot = structuredClone(mockSnapshot);
  browserRecordings = [];
  browserInstalledWhisperModels = new Set();
  browserInstalledSummaryModels = new Set();
  browserRecordingClock = null;
  browserHighlights = [];
  browserOakOsConnected = false;
  browserCloudKeys.clear();
  browserSessionListeners.clear();
  browserRecordingFinalizedListeners.clear();
  browserRecordingUpdatedListeners.clear();
  browserAutomaticListeners.clear();
  browserAutomaticStatus = null;
  browserAutomaticQueue = Promise.resolve();
  browserGeneration += 1;
}

export async function getCloudKeyStatuses(): Promise<CloudKeyStatus[]> {
  if (isTauri()) return command<CloudKeyStatus[]>("get_cloud_key_statuses");
  return (["openai", "anthropic"] as const).map((provider) => ({ provider, keyStored: browserCloudKeys.has(provider) }));
}

export async function saveCloudApiKey(provider: CloudProvider, apiKey: string): Promise<void> {
  if (isTauri()) return command<void>("save_cloud_api_key", { provider, apiKey });
  // Browser preview retains only presence, never the supplied credential.
  if (!apiKey.trim() || apiKey.length > 4096 || /[\x00-\x1f\x7f]/.test(apiKey.trim())) throw new Error("Enter a valid API key");
  browserCloudKeys.add(provider);
}

export async function removeCloudApiKey(provider: CloudProvider): Promise<void> {
  if (isTauri()) return command<void>("remove_cloud_api_key", { provider });
  browserCloudKeys.delete(provider);
}

export async function onRecordingUpdated(handler: (recording: Recording) => void): Promise<UnlistenFn> {
  if (!isTauri()) {
    browserRecordingUpdatedListeners.add(handler);
    return () => browserRecordingUpdatedListeners.delete(handler);
  }
  return listen<Recording>("recording-updated", (event) => handler(event.payload));
}

export async function onAutomaticProcessing(handler: (status: AutomaticProcessingStatus) => void): Promise<UnlistenFn> {
  if (!isTauri()) {
    browserAutomaticListeners.add(handler);
    return () => browserAutomaticListeners.delete(handler);
  }
  return listen<AutomaticProcessingStatus>("automatic-processing", (event) => handler(event.payload));
}

export async function getAutomaticProcessingStatus(): Promise<AutomaticProcessingStatus | null> {
  if (isTauri()) return command<AutomaticProcessingStatus | null>("get_automatic_processing_status");
  return structuredClone(browserAutomaticStatus);
}
