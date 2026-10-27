// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import * as api from "./api";
import type { AutomaticProcessingStatus } from "./types";
import "../tests/selectSetup";

beforeEach(api.resetBrowserMock);
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
async function record() { await api.startRecording({ mode: "online" }); return api.stopRecording(); }
async function settings(tab: string) {
  await api.completeOnboarding({ launchAtLogin: false });
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
  fireEvent.click(screen.getByRole("tab", { name: tab }));
}
async function finished(kind: "transcription" | "summary") {
  await waitFor(async () => {
    const status = await api.getAutomaticProcessingStatus();
    expect(status?.kind).toBe(kind);
    expect(status?.active).toBe(false);
  });
}

describe("automatic processing settings", () => {
  it("has independent toggles in the relevant tabs, disabled by default and saved across remounts", async () => {
    await settings("Transcription");
    const transcribe = screen.getByRole("checkbox", { name: /^Auto transcribe/ }) as HTMLInputElement;
    expect(transcribe.checked).toBe(false);
    expect(screen.queryByRole("checkbox", { name: /^Auto summarize/ })).toBeNull();
    fireEvent.click(transcribe);
    await waitFor(async () => expect((await api.getSnapshot()).settings.autoTranscribe).toBe(true));
    fireEvent.click(screen.getByRole("tab", { name: "Summaries" }));
    const summarize = screen.getByRole("checkbox", { name: /^Auto summarize/ }) as HTMLInputElement;
    expect(summarize.checked).toBe(false);
    fireEvent.click(summarize);
    await waitFor(async () => expect((await api.getSnapshot()).settings.autoSummarize).toBe(true));
    cleanup();
    await settings("Transcription");
    expect((screen.getByRole("checkbox", { name: /^Auto transcribe/ }) as HTMLInputElement).checked).toBe(true);
    fireEvent.click(screen.getByRole("checkbox", { name: /^Auto transcribe/ }));
    await waitFor(async () => expect((await api.getSnapshot()).settings.autoTranscribe).toBe(false));
    expect((await api.getSnapshot()).settings.autoSummarize).toBe(true);
  });

  it("explains automatic cloud uploads and keeps the toggle off when saving fails", async () => {
    await api.updateSettings({ transcriptionSource: "cloud" });
    await settings("Transcription");
    expect(screen.getByText(/Audio is sent to OpenAI automatically/)).toBeTruthy();
    vi.spyOn(api, "updateSettings").mockRejectedValue(new Error("Settings could not be saved"));
    fireEvent.click(screen.getByRole("checkbox", { name: /^Auto transcribe/ }));
    expect((await screen.findByRole("alert")).textContent).toContain("Settings could not be saved");
    expect((screen.getByRole("checkbox", { name: /^Auto transcribe/ }) as HTMLInputElement).checked).toBe(false);
  });
});

describe("automatic recording workflow", () => {
  it("does nothing by default and does not backfill older recordings when enabled", async () => {
    const recording = await record();
    await api.updateSettings({ autoTranscribe: true, autoSummarize: true });
    expect((await api.listRecordings()).find((item) => item.id === recording.id)?.transcript).toBeNull();
    expect(await api.getAutomaticProcessingStatus()).toBeNull();
  });

  it("transcribes then summarizes new recordings using the saved defaults and template", async () => {
    await api.updateSettings({ autoTranscribe: true, autoSummarize: true, whisperModelPath: "/model.bin", summaryModelPath: "/summary.gguf", summaryTemplate: "actions" });
    const events: AutomaticProcessingStatus[] = [];
    await api.onAutomaticProcessing((status) => events.push(status));
    const recording = await record();
    await finished("summary");
    const saved = (await api.listRecordings()).find((item) => item.id === recording.id)!;
    expect(saved.transcript?.source).toBe("local");
    expect(saved.summary?.templateId).toBe("actions");
    expect(events.map((status) => `${status.kind}:${status.active}`)).toEqual(["transcription:true", "transcription:false", "summary:true", "summary:false"]);
    expect(events.some((status) => status.error)).toBe(false);
  });

  it("auto summarizes manual cloud transcription independently of auto transcribe", async () => {
    await api.saveCloudApiKey("openai", "test-key");
    await api.saveCloudApiKey("anthropic", "test-key");
    await api.updateSettings({ autoSummarize: true, transcriptionSource: "cloud", summarySource: "cloud", cloudProvider: "anthropic", cloudModel: "test-model" });
    const recording = await record();
    expect(await api.getAutomaticProcessingStatus()).toBeNull();
    await api.transcribeRecording(recording.id);
    await finished("summary");
    const saved = (await api.listRecordings())[0];
    expect(saved.transcript?.source).toBe("cloud");
    expect(saved.summary?.model).toBe("Cloud · Anthropic / test-model");
  });

  it("reports missing cloud keys without attempting a summary or losing the recording", async () => {
    await api.updateSettings({ autoTranscribe: true, autoSummarize: true, transcriptionSource: "cloud" });
    await api.completeOnboarding({ launchAtLogin: false });
    render(<App />);
    await screen.findByRole("button", { name: "Settings" });
    await record();
    await finished("transcription");
    expect((await api.getAutomaticProcessingStatus())?.error).toContain("OpenAI API key");
    expect((await screen.findByRole("alert")).textContent).toContain("Auto transcription failed");
    const saved = (await api.listRecordings())[0];
    expect(saved.transcript).toBeNull();
    expect(saved.summary).toBeNull();
  });

  it("keeps a successful transcript when the summary model is missing", async () => {
    await api.updateSettings({ autoTranscribe: true, autoSummarize: true, whisperModelPath: "/model.bin" });
    await record();
    await finished("summary");
    expect((await api.getAutomaticProcessingStatus())?.error).toContain("summary model");
    const saved = (await api.listRecordings())[0];
    expect(saved.transcript?.text).toBeTruthy();
    expect(saved.summary).toBeNull();
  });
});
