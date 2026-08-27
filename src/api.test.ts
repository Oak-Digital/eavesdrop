// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  addHighlight,
  deleteRecordings,
  getSnapshot,
  installWhisperModel,
  listRecordings,
  listWhisperModels,
  onRecordingFinalized,
  onSessionChanged,
  pauseRecording,
  resetBrowserMock,
  removeWhisperModel,
  resumeRecording,
  restoreRecordings,
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
});

describe("bulk recording actions", () => {
  beforeEach(resetBrowserMock);

  it("moves and restores multiple recordings together", async () => {
    await startRecording({ mode: "in_person" });
    const first = await stopRecording();
    await startRecording({ mode: "online" });
    const second = await stopRecording();

    await deleteRecordings([first.id, second.id]);
    expect(await listRecordings(false)).toHaveLength(0);
    expect(await listRecordings(true)).toHaveLength(2);

    await restoreRecordings([first.id, second.id]);
    expect(await listRecordings(false)).toHaveLength(2);
    expect(await listRecordings(true)).toHaveLength(0);
  });
});

describe("summary prompt settings", () => {
  beforeEach(resetBrowserMock);

  it("stores a custom summary prompt", async () => {
    await updateSettings({ summaryPrompt: "Focus on risks and unanswered questions." });
    expect((await getSnapshot()).settings.summaryPrompt).toBe("Focus on risks and unanswered questions.");
  });
});

describe("summary titles", () => {
  beforeEach(resetBrowserMock);

  it("renames a recording to the generated summary title", async () => {
    await startRecording({ mode: "in_person" });
    const recording = await stopRecording();

    const summarized = await summarizeRecording(recording.id);

    expect(summarized.title).toBe(summarized.summary?.suggestedTitle);
    expect((await listRecordings(false))[0].title).toBe("Example local summary");
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
