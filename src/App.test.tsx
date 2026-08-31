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
  activity: null,
  onChooseWhisperModel: async () => undefined,
  onChooseSummaryModel: async () => undefined,
  onBack: () => undefined,
  onJobStart: () => true,
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

describe("RecordingDetail processing jobs", () => {
  const processedRecording: Recording = {
    ...recording,
    transcript: {
      text: "An existing transcript.",
      language: "English",
      createdAt: "2026-01-01T12:31:00Z",
      segments: [{ startMs: 0, endMs: 2_000, text: "An existing transcript." }],
    },
  };

  it("disables both processing actions while either job is active", () => {
    render(<RecordingDetail
      recording={processedRecording}
      {...baseProps}
      whisperModelPath="/models/whisper.bin"
      summaryModelPath="/models/summary.gguf"
      activity={{ kind: "summary", recordingId: recording.id, title: recording.title, label: "Writing the summary", progress: 0.5 }}
    />);

    expect((screen.getByRole("button", { name: "Summarizing…" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Transcribe again" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("applies the processing lock across recordings", () => {
    render(<RecordingDetail
      recording={processedRecording}
      {...baseProps}
      whisperModelPath="/models/whisper.bin"
      summaryModelPath="/models/summary.gguf"
      activity={{ kind: "transcription", recordingId: "another-recording", title: "Another recording", label: "Transcribing", progress: 0.25 }}
    />);

    expect((screen.getByRole("button", { name: "Summarize" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Transcribe again" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("does not start work when the shared activity lock rejects it", () => {
    const transcribe = vi.spyOn(api, "transcribeRecording");
    const onJobStart = vi.fn(() => false);
    render(<RecordingDetail
      recording={processedRecording}
      {...baseProps}
      whisperModelPath="/models/whisper.bin"
      onJobStart={onJobStart}
    />);

    fireEvent.click(screen.getByRole("button", { name: "Transcribe again" }));

    expect(onJobStart).toHaveBeenCalledWith(processedRecording, "transcription", "Preparing audio");
    expect(transcribe).not.toHaveBeenCalled();
  });
});

describe("RecordingDetail OakOS publishing", () => {
  it("requires a fresh project choice before publishing", async () => {
    vi.spyOn(api, "getOakOsIntegration").mockResolvedValue({ connected: true });
    vi.spyOn(api, "listOakOsProjects").mockResolvedValue([
      { id: "project-one", name: "Product" },
      { id: "project-two", name: "Research" },
    ]);
    const publish = vi.spyOn(api, "publishRecordingToOakOs").mockResolvedValue({ location: "/api/v1/recordings/remote" });
    render(<RecordingDetail recording={recording} {...baseProps} />);

    fireEvent.click(screen.getByRole("button", { name: "Publish to OakOS" }));
    const projectSelect = await screen.findByLabelText("Project");
    expect((projectSelect as HTMLSelectElement).value).toBe("");
    fireEvent.change(projectSelect, { target: { value: "project-one" } });
    fireEvent.click(screen.getByRole("button", { name: "Publish" }));

    await waitFor(() => expect(publish).toHaveBeenCalledWith(recording.id, "project-one"));
    expect((await screen.findByRole("status")).textContent).toContain("Sent to Product");
  });

  it("opens integration setup when OakOS is not connected", async () => {
    vi.spyOn(api, "getOakOsIntegration").mockResolvedValue({ connected: false });
    const onOpenIntegrations = vi.fn();
    render(<RecordingDetail recording={recording} {...baseProps} onOpenIntegrations={onOpenIntegrations} />);

    fireEvent.click(screen.getByRole("button", { name: "Publish to OakOS" }));

    await waitFor(() => expect(onOpenIntegrations).toHaveBeenCalledOnce());
  });
});
