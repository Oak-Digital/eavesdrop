// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as api from "./api";
import { RecordingDetail } from "./App";
import type { Recording } from "./types";

const recording: Recording = {
  id: "recording-one",
  title: "Weekly planning",
  mode: "online",
  startedAt: "2026-01-01T12:00:00Z",
  endedAt: "2026-01-01T12:30:00Z",
  durationMs: 1_800_000,
  playableDurationMs: 1_800_000,
  status: "ready",
  sizeBytes: 1024,
  codec: "AAC-LC",
  detectedApp: null,
  deletedAt: null,
  highlights: [],
  transcript: null,
  summary: null,
};

const baseProps = {
  deleted: false,
  whisperModelPath: null,
  summaryModelPath: null,
  onChooseWhisperModel: async () => undefined,
  onChooseSummaryModel: async () => undefined,
  onBack: () => undefined,
  onJobStart: () => undefined,
  onJobEnd: () => undefined,
  onChanged: () => undefined,
  onRemoved: () => undefined,
};

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("RecordingDetail title editing", () => {
  it("restores the saved title when the input is blank", () => {
    render(<RecordingDetail recording={recording} {...baseProps} />);
    const input = screen.getByLabelText("Recording title") as HTMLInputElement;

    fireEvent.change(input, { target: { value: "   " } });
    fireEvent.blur(input);

    expect(input.value).toBe(recording.title);
  });

  it("recovers from a failed rename and remains editable", async () => {
    vi.spyOn(api, "renameRecording").mockRejectedValueOnce(new Error("The library is temporarily unavailable."));
    render(<RecordingDetail recording={recording} {...baseProps} />);
    const input = screen.getByLabelText("Recording title") as HTMLInputElement;

    fireEvent.change(input, { target: { value: "New planning title" } });
    fireEvent.blur(input);

    await waitFor(() => expect(screen.getByRole("alert").textContent).toBe("The library is temporarily unavailable."));
    expect(input.value).toBe(recording.title);
    expect(input.readOnly).toBe(false);
  });

  it("synchronizes the field when another recording is selected", () => {
    const { rerender } = render(<RecordingDetail recording={recording} {...baseProps} />);
    const input = screen.getByLabelText("Recording title") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "Unsaved draft" } });

    const nextRecording = { ...recording, id: "recording-two", title: "Customer interview" };
    rerender(<RecordingDetail recording={nextRecording} {...baseProps} />);

    expect(input.value).toBe(nextRecording.title);
  });
});
