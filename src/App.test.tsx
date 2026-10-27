// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as api from "./api";
import { RecordingDetail } from "./App";
import type { Recording } from "./types";
import "../tests/selectSetup";

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
    expect(projectSelect.textContent).toBe("Choose a project…");
    expect((screen.getByRole("button", { name: "Publish" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.keyDown(projectSelect, { key: "Enter" });
    fireEvent.click(await screen.findByRole("option", { name: "Product" }));
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

describe("summary templates", () => {
  const transcribed: Recording = { ...recording, transcript: { text: "Plan a pilot.", language: "Danish", createdAt: "now", segments: [] } };

  it("defaults to discussion and sends the chosen template with generation", async () => {
    const summarize = vi.spyOn(api, "summarizeRecording").mockResolvedValue(transcribed);
    const update = vi.spyOn(api, "updateSettings");
    render(<RecordingDetail recording={transcribed} {...baseProps} summaryModelPath="/models/local.gguf" />);
    const picker = screen.getByRole("combobox", { name: "Summary template" });
    expect(picker.textContent).toContain("Discussion summary");
    fireEvent.keyDown(picker, { key: "Enter" });
    fireEvent.click(await screen.findByRole("option", { name: /Action items/ }));
    fireEvent.click(screen.getByRole("button", { name: "Summarize" }));
    await waitFor(() => expect(summarize).toHaveBeenCalledWith(recording.id, "actions", { source: "local" }));
    expect(update).not.toHaveBeenCalled();
  });

  it("uses the saved default for new summaries and the original template for regeneration", () => {
    const { rerender } = render(<RecordingDetail recording={transcribed} {...baseProps} defaultSummaryTemplate="concise" />);
    expect(screen.getByRole("combobox", { name: "Summary template" }).textContent).toContain("Concise summary");
    const summarized = { ...transcribed, summary: { suggestedTitle: recording.title, overview: "", keyPoints: [], decisions: [], actionItems: [], model: "local", createdAt: "now", templateId: "discussion", markdown: "# Pilot\n\n## 1. Scope\n\n### Konklusion\nA pilot was proposed.\n\n- **Concern:** No owner.\n\n## Uafklarede forhold\nTiming remains open." } };
    rerender(<RecordingDetail recording={summarized} {...baseProps} defaultSummaryTemplate="concise" />);
    expect(screen.getByRole("combobox", { name: "Summary template" }).textContent).toContain("Discussion summary");
    expect(screen.getByRole("heading", { name: "1. Scope" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Konklusion" })).toBeTruthy();
    expect(screen.getByText("Concern:").tagName).toBe("STRONG");
    expect(screen.queryByRole("heading", { name: "Key points" })).toBeNull();
  });

  it("retains legacy summaries and disables the picker during processing", () => {
    render(<RecordingDetail recording={{ ...transcribed, summary: { suggestedTitle: recording.title, overview: "Existing summary", keyPoints: ["Existing point"], decisions: [], actionItems: [], model: "local", createdAt: "now" } }} {...baseProps}
      activity={{ kind: "summary", recordingId: recording.id, title: recording.title, label: "Writing", progress: 0.5 }} />);
    expect(screen.getByText("Existing summary")).toBeTruthy();
    expect(screen.getByText("Existing point")).toBeTruthy();
    expect((screen.getByRole("combobox", { name: "Summary template" }) as HTMLButtonElement).disabled).toBe(true);
  });
});

describe("Local and Cloud summary processing", () => {
  const transcribed: Recording = { ...recording, transcript: { text: "Discussed a pilot.", language: "en", createdAt: "now", segments: [] } };
  it("generates with Cloud without a local model and passes the explicit provider and model", async () => {
    const summarize = vi.spyOn(api, "summarizeRecording").mockResolvedValue(transcribed);
    render(<RecordingDetail recording={transcribed} {...baseProps} defaultSummarySource="cloud" cloudProvider="anthropic" cloudModel="claude-test" cloudKeyStored />);
    expect(screen.getByText(/The transcript will be sent to Anthropic/).textContent).toContain("Audio stays local");
    fireEvent.click(screen.getByRole("button", { name: "Summarize with Cloud" }));
    await waitFor(() => expect(summarize).toHaveBeenCalledWith(recording.id, "discussion", { source: "cloud", provider: "anthropic", model: "claude-test" }));
  });

  it("requires the selected Cloud key and allows an explicit return to Local", async () => {
    const summarize = vi.spyOn(api, "summarizeRecording"); const settings = vi.fn();
    render(<RecordingDetail recording={transcribed} {...baseProps} defaultSummarySource="cloud" summaryModelPath="/models/local.gguf" onOpenSummarySettings={settings} />);
    fireEvent.click(screen.getByRole("button", { name: "Set up Cloud" })); expect(settings).toHaveBeenCalledOnce();
    expect(summarize).not.toHaveBeenCalled();
    fireEvent.keyDown(screen.getByRole("combobox", { name: "Summary processing" }), { key: "Enter" });
    fireEvent.click(await screen.findByRole("option", { name: /Local/ }));
    expect(screen.getByRole("button", { name: "Summarize" })).toBeTruthy();
    expect(screen.getByText(/The transcript is processed on this computer/)).toBeTruthy();
  });

  it("keeps the previous summary visible after a Cloud failure", async () => {
    vi.spyOn(api, "summarizeRecording").mockRejectedValue(new Error("Provider quota reached"));
    const onChanged = vi.fn(); const onJobEnd = vi.fn();
    const summarized = { ...transcribed, summary: { suggestedTitle: "Pilot", overview: "Previous summary", keyPoints: [], decisions: [], actionItems: [], model: "local.gguf", createdAt: "now" } };
    render(<RecordingDetail recording={summarized} {...baseProps} defaultSummarySource="cloud" cloudKeyStored onChanged={onChanged} onJobEnd={onJobEnd} />);
    fireEvent.click(screen.getByRole("button", { name: "Summarize again with Cloud" }));
    expect((await screen.findByRole("alert")).textContent).toBe("Provider quota reached");
    expect(screen.getByText("Previous summary")).toBeTruthy(); expect(onChanged).not.toHaveBeenCalled();
    expect(onJobEnd).toHaveBeenCalledWith(recording.id, "summary");
  });
});
