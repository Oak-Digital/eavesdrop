// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  addHighlight,
  connectOakOs,
  deleteRecordings,
  disconnectOakOs,
  getSnapshot,
  installWhisperModel,
  listRecordings,
  listOakOsProjects,
  listWhisperModels,
  onRecordingFinalized,
  onSessionChanged,
  pauseRecording,
  publishRecordingToOakOs,
  resetBrowserMock,
  removeWhisperModel,
  resumeRecording,
  setRecordingMode,
  startRecording,
  stopRecording,
  summarizeRecording,
  updateSettings,
  useWhisperModel,
} from "./api";

describe("browser recording lifecycle", () => {
  beforeEach(() => {
    resetBrowserMock();
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-01T12:00:00Z"));
  });

  afterEach(() => vi.useRealTimers());

  it("preserves playable time across pause and resume", async () => {
    await startRecording({ mode: "online" });
    vi.advanceTimersByTime(2_500);

    const paused = await pauseRecording();
    expect(paused.elapsedMs).toBe(2_500);
    expect(paused.playableMs).toBe(2_500);

    vi.advanceTimersByTime(1_500);
    const resumed = await resumeRecording();
    expect(resumed.elapsedMs).toBe(4_000);
    expect(resumed.playableMs).toBe(2_500);

    vi.advanceTimersByTime(1_000);
    await addHighlight();
    const recording = await stopRecording();

    expect(recording.durationMs).toBe(5_000);
    expect(recording.playableDurationMs).toBe(3_500);
    expect(recording.highlights[0]?.offsetMs).toBe(3_500);
  });

  it("emits the desktop lifecycle events and returns to idle after stopping", async () => {
    const phases: string[] = [];
    const finalized: string[] = [];
    const stopSessionListener = await onSessionChanged((session) => phases.push(session.phase));
    const stopFinalizedListener = await onRecordingFinalized((recording) => finalized.push(recording.id));

    const started = await startRecording({ mode: "in_person" });
    await pauseRecording();
    await resumeRecording();
    const recording = await stopRecording();

    expect(recording.id).toBe(started.recordingId);
    expect(phases).toEqual(["recording", "paused", "recording", "finalizing", "idle"]);
    expect(finalized).toEqual([recording.id]);
    expect((await getSnapshot()).session.phase).toBe("idle");

    stopSessionListener();
    stopFinalizedListener();
  });

  it("changes sources in the same recording and preserves paused time and highlights", async () => {
    const started = await startRecording({ mode: "in_person" });
    vi.advanceTimersByTime(2_000);
    await addHighlight();
    const online = await setRecordingMode("online");
    expect(online.recordingId).toBe(started.recordingId);
    expect(online.startedAt).toBe(started.startedAt);
    expect(online.playableMs).toBe(2_000);
    expect(online.phase).toBe("recording");
    expect(online.mode).toBe("online");
    await pauseRecording();
    vi.advanceTimersByTime(1_000);
    const microphone = await setRecordingMode("in_person");
    expect(microphone.phase).toBe("paused");
    expect(microphone.playableMs).toBe(2_000);
    expect(microphone.mode).toBe("in_person");
    await resumeRecording();
    vi.advanceTimersByTime(1_000);
    const saved = await stopRecording();
    expect(saved.id).toBe(started.recordingId);
    expect(saved.playableDurationMs).toBe(3_000);
    expect(saved.mode).toBe("in_person");
    expect(saved.highlights[0]?.offsetMs).toBe(2_000);
    expect(await listRecordings()).toHaveLength(1);
    await expect(setRecordingMode("online")).rejects.toThrow("active recording");
  });
});

describe("bulk recording actions", () => {
  beforeEach(resetBrowserMock);

  it("permanently removes only the selected recordings", async () => {
    await startRecording({ mode: "in_person" });
    const first = await stopRecording();
    await startRecording({ mode: "online" });
    const second = await stopRecording();
    await startRecording({ mode: "online" });
    const keep = await stopRecording();

    await deleteRecordings([first.id, second.id]);
    expect((await listRecordings()).map((item) => item.id)).toEqual([keep.id]);
    await expect(deleteRecordings([first.id])).rejects.toThrow("Recording not found");
  });

  it("validates the entire selection before deleting", async () => {
    await startRecording({ mode: "online" });
    const recording = await stopRecording();
    await expect(deleteRecordings([recording.id, "missing"])).rejects.toThrow("Recording not found");
    expect(await listRecordings()).toHaveLength(1);
  });
});

describe("summary prompt settings", () => {
  beforeEach(resetBrowserMock);

  it("stores a custom summary prompt", async () => {
    await updateSettings({ summaryPrompt: "Focus on risks and unanswered questions." });
    expect((await getSnapshot()).settings.summaryPrompt).toBe("Focus on risks and unanswered questions.");
  });
});

describe("appearance settings", () => {
  beforeEach(resetBrowserMock);

  it("stores a light theme preference", async () => {
    await updateSettings({ theme: "light" });
    expect((await getSnapshot()).settings.theme).toBe("light");
  });
});

describe("OakOS integration", () => {
  beforeEach(resetBrowserMock);

  it("connects, selects a project, and publishes a recording", async () => {
    await connectOakOs("oakos_pat_test");
    const [project] = await listOakOsProjects();
    await startRecording({ mode: "online" });
    const recording = await stopRecording();

    await expect(publishRecordingToOakOs(recording.id, project.id)).resolves.toEqual({
      location: `/api/v1/recordings/${recording.id}`,
    });
  });

  it("removes access to projects when disconnected", async () => {
    await connectOakOs("oakos_pat_test");

    await disconnectOakOs();

    await expect(listOakOsProjects()).rejects.toThrow("OakOS is not connected");
  });
});

describe("summary titles", () => {
  beforeEach(resetBrowserMock);

  it("renames a recording to the generated summary title", async () => {
    await startRecording({ mode: "in_person" });
    const recording = await stopRecording();

    const summarized = await summarizeRecording(recording.id);

    expect(summarized.title).toBe(summarized.summary?.suggestedTitle);
    expect((await listRecordings())[0].title).toBe("Example local summary");
  });
});

describe("Whisper model installation", () => {
  beforeEach(resetBrowserMock);

  it("installs and selects a curated model", async () => {
    expect((await listWhisperModels()).find((model) => model.id === "base")?.installed).toBe(false);

    await installWhisperModel("base");

    expect((await listWhisperModels()).find((model) => model.id === "base")?.installed).toBe(true);
    expect((await getSnapshot()).settings.whisperModelPath).toBe("/models/ggml-base.bin");
  });

  it("removes an installed model and clears it when selected", async () => {
    await installWhisperModel("base");

    await removeWhisperModel("base");

    expect((await listWhisperModels()).find((model) => model.id === "base")?.installed).toBe(false);
    expect((await getSnapshot()).settings.whisperModelPath).toBeNull();
  });

  it("selects an already-installed model without installing it again", async () => {
    await installWhisperModel("base");
    await installWhisperModel("tiny");

    await useWhisperModel("base");

    expect((await getSnapshot()).settings.whisperModelPath).toBe("/models/ggml-base.bin");
  });
});
