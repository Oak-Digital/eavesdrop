// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App, { RecordingDetail } from "./App";
import { CloudTranscriptionSettings } from "./CloudTranscriptionSettings";
import * as api from "./api";
import type { Recording } from "./types";
import "../tests/selectSetup";

beforeEach(api.resetBrowserMock);
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
const baseProps = {
  whisperModelPath: null, summaryModelPath: null, activity: null,
  onChooseWhisperModel: async () => {}, onChooseSummaryModel: async () => {},
  onBack: () => {}, onJobStart: () => true, onJobEnd: () => {}, onChanged: () => {}, onRemoved: () => {},
};
async function savedRecording(): Promise<Recording> {
  await api.startRecording({ mode: "online" });
  return { ...await api.stopRecording(), sizeBytes: 1000 };
}
async function choose(label: string, name: RegExp) {
  fireEvent.keyDown(screen.getByRole("combobox", { name: label }), { key: "Enter" });
  fireEvent.click(await screen.findByRole("option", { name }));
}

describe("cloud transcription controls", () => {
  it("transcribes without a local model and displays plain text and cloud provenance", async () => {
    await api.saveCloudApiKey("openai", "test-key");
    const recording = await savedRecording();
    const transcribe = vi.spyOn(api, "transcribeRecording");
    const onChanged = vi.fn();
    const onJobEnd = vi.fn();
    const { rerender } = render(<RecordingDetail recording={recording} {...baseProps} defaultTranscriptionSource="cloud" transcriptionKeyStored onChanged={onChanged} onJobEnd={onJobEnd} />);
    expect(screen.getByText(/Audio will be sent to OpenAI when you transcribe/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Transcribe with Cloud" }));
    await waitFor(() => expect(onChanged).toHaveBeenCalledOnce());
    expect(transcribe).toHaveBeenCalledWith(recording.id, { source: "cloud" });
    expect(onJobEnd).toHaveBeenCalledWith(recording.id, "transcription");
    const changed = onChanged.mock.calls[0][0] as Recording;
    expect(changed.transcript?.segments).toEqual([]);
    rerender(<RecordingDetail recording={changed} {...baseProps} defaultTranscriptionSource="cloud" transcriptionKeyStored />);
    expect(screen.getByText("Example cloud transcript.")).toBeTruthy();
    expect(screen.getByText(/Cloud · OpenAI \/ gpt-transcribe · English/)).toBeTruthy();
  });

  it("requires the OpenAI transcription key even if a different summary key is available", async () => {
    const setup = vi.fn();
    const transcribe = vi.spyOn(api, "transcribeRecording");
    render(<RecordingDetail recording={await savedRecording()} {...baseProps} cloudProvider="anthropic" cloudKeyStored defaultTranscriptionSource="cloud" onOpenTranscriptionSettings={setup} />);
    expect(screen.queryByRole("button", { name: "Transcribe with Cloud" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Set up cloud transcription" }));
    expect(setup).toHaveBeenCalledOnce();
    expect(transcribe).not.toHaveBeenCalled();
  });

  it("lets one recording use Local without changing the saved cloud default", async () => {
    await api.updateSettings({ transcriptionSource: "cloud" });
    const recording = await savedRecording();
    const transcribe = vi.spyOn(api, "transcribeRecording");
    const onChanged = vi.fn();
    render(<RecordingDetail recording={recording} {...baseProps} defaultTranscriptionSource="cloud" transcriptionKeyStored whisperModelPath="/models/whisper.bin" onChanged={onChanged} />);
    await choose("Transcription processing", /Local/);
    fireEvent.click(screen.getByRole("button", { name: "Transcribe" }));
    await waitFor(() => expect(onChanged).toHaveBeenCalledOnce());
    expect(transcribe).toHaveBeenCalledWith(recording.id, { source: "local" });
    expect((await api.getSnapshot()).settings.transcriptionSource).toBe("cloud");
  });

  it("preserves the existing transcript when cloud transcription fails", async () => {
    const recording = await savedRecording();
    recording.transcript = { text: "Existing transcript", language: null, createdAt: "now", segments: [] };
    vi.spyOn(api, "transcribeRecording").mockRejectedValue(new Error("OpenAI quota reached"));
    const onChanged = vi.fn(); const onJobEnd = vi.fn();
    render(<RecordingDetail recording={recording} {...baseProps} defaultTranscriptionSource="cloud" transcriptionKeyStored onChanged={onChanged} onJobEnd={onJobEnd} />);
    fireEvent.click(screen.getByRole("button", { name: "Transcribe again with Cloud" }));
    expect((await screen.findByRole("alert")).textContent).toBe("OpenAI quota reached");
    expect(screen.getByText("Existing transcript")).toBeTruthy();
    expect(onChanged).not.toHaveBeenCalled();
    expect(onJobEnd).toHaveBeenCalledWith(recording.id, "transcription");
  });
});

describe("cloud transcription settings", () => {
  it("saves credentials separately and clears the key input without changing defaults", async () => {
    const onSave = vi.fn(); const onKeysChanged = vi.fn().mockResolvedValue(undefined);
    const saveKey = vi.spyOn(api, "saveCloudApiKey");
    render(<CloudTranscriptionSettings settings={(await api.getSnapshot()).settings} keys={[]} keyError={null} onSave={onSave} onKeysChanged={onKeysChanged} />);
    fireEvent.click(screen.getByText("Cloud transcription", { selector: "summary" }));
    const key = screen.getByLabelText("OpenAI transcription API key") as HTMLInputElement;
    expect(key.type).toBe("password");
    expect((screen.getByRole("button", { name: "Use cloud transcription" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(key, { target: { value: "dummy-test-key" } });
    fireEvent.click(screen.getByRole("button", { name: "Save key" }));
    await waitFor(() => expect(onKeysChanged).toHaveBeenCalledOnce());
    expect(saveKey).toHaveBeenCalledWith("openai", "dummy-test-key");
    expect(key.value).toBe("");
    expect(onSave).not.toHaveBeenCalled();
    expect(JSON.stringify((await api.getSnapshot()).settings)).not.toContain("dummy-test-key");
  });

  it("uses an existing OpenAI key and changes transcription independently of summary processing", async () => {
    await api.completeOnboarding({ launchAtLogin: false });
    await api.saveCloudApiKey("openai", "test-key");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    fireEvent.click(screen.getByRole("tab", { name: "Transcription" }));
    await choose("Default transcription processing", /Cloud/);
    await screen.findByText(/OpenAI key saved/);
    const settings = (await api.getSnapshot()).settings;
    expect(settings.transcriptionSource).toBe("cloud");
    expect(settings.summarySource).toBe("local");
    expect(screen.getByText(/This key is shared with OpenAI summaries/)).toBeTruthy();
  });

  it("does not claim a key was saved when the credential store fails", async () => {
    vi.spyOn(api, "saveCloudApiKey").mockRejectedValue(new Error("Credential store unavailable"));
    render(<CloudTranscriptionSettings settings={(await api.getSnapshot()).settings} keys={[]} keyError={null} onSave={vi.fn()} onKeysChanged={vi.fn()} />);
    fireEvent.click(screen.getByText("Cloud transcription", { selector: "summary" }));
    fireEvent.change(screen.getByLabelText("OpenAI transcription API key"), { target: { value: "dummy-key" } });
    fireEvent.click(screen.getByRole("button", { name: "Save key" }));
    expect((await screen.findByRole("alert")).textContent).toBe("Credential store unavailable");
    expect(screen.queryByText(/^OpenAI key saved/)).toBeNull();
  });
});

describe("cloud transcription API routing", () => {
  it("defaults to Local, honors the saved Cloud default, and preserves text after key removal", async () => {
    const recording = await savedRecording();
    expect((await api.transcribeRecording(recording.id)).transcript?.source).toBe("local");
    await api.saveCloudApiKey("openai", "test-key");
    await api.updateSettings({ transcriptionSource: "cloud" });
    const cloud = await api.transcribeRecording(recording.id);
    expect(cloud.transcript?.source).toBe("cloud");
    await api.removeCloudApiKey("openai");
    await expect(api.transcribeRecording(recording.id)).rejects.toThrow("OpenAI API key");
    expect((await api.listRecordings())[0].transcript).toEqual(cloud.transcript);
  });
});
