import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import type {
  AppSettings,
  AppSnapshot,
  AudioLevels,
  MeetingCandidate,
  Recording,
  RecordingSession,
  StartRecordingInput,
} from "./types";

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
    microphoneId: null,
  },
  devices: [{ id: "default", name: "Default microphone", isDefault: true }],
};

let browserSnapshot = structuredClone(mockSnapshot);
let browserRecordings: Recording[] = [];

async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) throw new Error(`Command ${name} is only available in the desktop app`);
  return invoke<T>(name, args);
}

export async function getSnapshot(): Promise<AppSnapshot> {
  return isTauri() ? command<AppSnapshot>("get_app_snapshot") : structuredClone(browserSnapshot);
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

export async function startRecording(input: StartRecordingInput): Promise<RecordingSession> {
  if (isTauri()) return command<RecordingSession>("start_recording", { input });
  browserSnapshot.session = {
    ...idleSession,
    phase: "recording",
    recordingId: crypto.randomUUID(),
    mode: input.mode,
    startedAt: new Date().toISOString(),
  };
  return structuredClone(browserSnapshot.session);
}

export async function pauseRecording(): Promise<RecordingSession> {
  if (isTauri()) return command<RecordingSession>("pause_recording");
  browserSnapshot.session.phase = "paused";
  return structuredClone(browserSnapshot.session);
}

export async function resumeRecording(): Promise<RecordingSession> {
  if (isTauri()) return command<RecordingSession>("resume_recording");
  browserSnapshot.session.phase = "recording";
  return structuredClone(browserSnapshot.session);
}

export async function addHighlight(): Promise<RecordingSession> {
  if (isTauri()) return command<RecordingSession>("add_highlight");
  return structuredClone(browserSnapshot.session);
}

export async function stopRecording(): Promise<Recording> {
  if (isTauri()) return command<Recording>("stop_recording");
  const now = new Date().toISOString();
  const recording: Recording = {
    id: browserSnapshot.session.recordingId!,
    title: `${browserSnapshot.session.mode === "online" ? "Online" : "In-person"} meeting`,
    mode: browserSnapshot.session.mode!,
    startedAt: browserSnapshot.session.startedAt!,
    endedAt: now,
    durationMs: browserSnapshot.session.elapsedMs,
    playableDurationMs: browserSnapshot.session.playableMs,
    status: "ready",
    sizeBytes: 0,
    codec: "AAC-LC",
    detectedApp: null,
    deletedAt: null,
    highlights: [],
  };
  browserRecordings.unshift(recording);
  browserSnapshot.session = structuredClone(idleSession);
  return recording;
}

export async function listRecordings(includeDeleted = false): Promise<Recording[]> {
  if (isTauri()) return command<Recording[]>("list_recordings", { includeDeleted });
  return structuredClone(browserRecordings.filter((item) => includeDeleted ? item.deletedAt : !item.deletedAt));
}

export async function renameRecording(id: string, title: string): Promise<Recording> {
  if (isTauri()) return command<Recording>("rename_recording", { id, title });
  const item = browserRecordings.find((recording) => recording.id === id)!;
  item.title = title;
  return structuredClone(item);
}

export async function deleteRecording(id: string): Promise<void> {
  if (isTauri()) return command<void>("delete_recording", { id });
  const item = browserRecordings.find((recording) => recording.id === id);
  if (item) item.deletedAt = new Date().toISOString();
}

export async function restoreRecording(id: string): Promise<void> {
  if (isTauri()) return command<void>("restore_recording", { id });
  const item = browserRecordings.find((recording) => recording.id === id);
  if (item) item.deletedAt = null;
}

export async function deleteRecordings(ids: string[]): Promise<void> {
  if (isTauri()) return command<void>("delete_recordings", { ids });
  const deletedAt = new Date().toISOString();
  browserRecordings.forEach((recording) => {
    if (ids.includes(recording.id)) recording.deletedAt = deletedAt;
  });
}

export async function restoreRecordings(ids: string[]): Promise<void> {
  if (isTauri()) return command<void>("restore_recordings", { ids });
  browserRecordings.forEach((recording) => {
    if (ids.includes(recording.id)) recording.deletedAt = null;
  });
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

export async function dismissMeeting(id: string): Promise<void> {
  if (isTauri()) await command<void>("dismiss_meeting", { id });
}

export async function openLibrary(recordingId?: string): Promise<void> {
  if (isTauri()) await command<void>("open_library", { recordingId });
}

export async function hideQuickPanel(): Promise<void> {
  if (isTauri()) await command<void>("hide_quick_panel");
}

export async function onSessionChanged(handler: (session: RecordingSession) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<RecordingSession>("recording-state-changed", (event) => handler(event.payload));
}

export async function onAudioLevels(handler: (levels: AudioLevels) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
  return listen<AudioLevels>("audio-levels", (event) => handler(event.payload));
}

export async function onRecordingFinalized(handler: (recording: Recording) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => undefined;
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
}
