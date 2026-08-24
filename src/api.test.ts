// @vitest-environment jsdom

import { beforeEach, describe, expect, it } from "vitest";
import {
  deleteRecordings,
  getSnapshot,
  installWhisperModel,
  listRecordings,
  listWhisperModels,
  resetBrowserMock,
  removeWhisperModel,
  restoreRecordings,
  startRecording,
  stopRecording,
} from "./api";

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
});
