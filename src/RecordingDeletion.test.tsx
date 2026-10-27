// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as dialog from "@tauri-apps/plugin-dialog";
import * as api from "./api";
import App, { RecordingDetail } from "./App";
import type { Recording } from "./types";
import "../tests/selectSetup";

vi.mock("@tauri-apps/plugin-dialog", async (importOriginal) => ({
  ...await importOriginal<typeof import("@tauri-apps/plugin-dialog")>(),
  confirm: vi.fn(),
}));

beforeEach(api.resetBrowserMock);
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

async function savedRecording(title: string): Promise<Recording> {
  await api.startRecording({ mode: "online" });
  const recording = await api.stopRecording();
  return api.renameRecording(recording.id, title);
}

async function renderDetail() {
  const recording = await savedRecording("Weekly planning");
  const onRemoved = vi.fn();
  render(<RecordingDetail recording={recording} whisperModelPath={null} summaryModelPath={null}
    activity={null} onChooseWhisperModel={async () => {}} onChooseSummaryModel={async () => {}}
    onBack={() => {}} onJobStart={() => true} onJobEnd={() => {}} onChanged={() => {}} onRemoved={onRemoved} />);
  return { recording, onRemoved };
}

describe("recording deletion confirmation", () => {
  it("uses a native warning dialog with explicit permanent-delete and cancel actions", async () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    const confirm = vi.mocked(dialog.confirm).mockResolvedValue(false);
    expect(await api.confirmRecordingDeletion([{ title: "Weekly planning" }])).toBe(false);
    expect(confirm).toHaveBeenCalledWith(expect.stringContaining("Permanently delete “Weekly planning”?"), {
      title: "Delete recording?", kind: "warning", okLabel: "Delete permanently", cancelLabel: "Cancel",
    });
    expect(confirm.mock.calls[0][0]).toContain("This cannot be undone.");
  });

  it("keeps the recording when the dialog is cancelled", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const { onRemoved } = await renderDetail();
    fireEvent.click(screen.getByRole("button", { name: "Delete recording" }));
    await waitFor(() => expect((screen.getByRole("button", { name: "Delete recording" }) as HTMLButtonElement).disabled).toBe(false));
    expect(confirm).toHaveBeenCalledWith(expect.stringContaining("Weekly planning"));
    expect(await api.listRecordings()).toHaveLength(1);
    expect(onRemoved).not.toHaveBeenCalled();
  });

  it("waits for confirmation before permanently deleting and leaving the detail view", async () => {
    let resolveConfirmation!: (value: boolean) => void;
    vi.spyOn(api, "confirmRecordingDeletion").mockReturnValue(new Promise((resolve) => { resolveConfirmation = resolve; }));
    const { recording, onRemoved } = await renderDetail();
    fireEvent.click(screen.getByRole("button", { name: "Delete recording" }));
    expect(await api.listRecordings()).toHaveLength(1);
    expect(onRemoved).not.toHaveBeenCalled();
    resolveConfirmation(true);
    await waitFor(() => expect(onRemoved).toHaveBeenCalledOnce());
    expect(await api.listRecordings()).toHaveLength(0);
    await expect(api.deleteRecording(recording.id)).rejects.toThrow("Recording not found");
  });

  it("shows cleanup errors without leaving the detail view and allows retry", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const remove = vi.spyOn(api, "deleteRecording").mockRejectedValueOnce(new Error("Could not delete recording file"));
    const { onRemoved } = await renderDetail();
    fireEvent.click(screen.getByRole("button", { name: "Delete recording" }));
    expect((await screen.findByRole("alert")).textContent).toBe("Could not delete recording file");
    expect(onRemoved).not.toHaveBeenCalled();
    expect(await api.listRecordings()).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "Delete recording" }));
    await waitFor(() => expect(onRemoved).toHaveBeenCalledOnce());
    expect(remove).toHaveBeenCalledTimes(2);
    expect(await api.listRecordings()).toHaveLength(0);
  });
});

describe("library bulk deletion", () => {
  it("removes Recently Deleted and confirms before deleting the selected recordings", async () => {
    await api.completeOnboarding({ launchAtLogin: false });
    await savedRecording("First meeting");
    await savedRecording("Second meeting");
    await savedRecording("Keep this meeting");
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<App />);
    await screen.findByRole("checkbox", { name: "Select First meeting" });
    expect(screen.queryByRole("button", { name: /Recently deleted/i })).toBeNull();
    fireEvent.click(screen.getByRole("checkbox", { name: "Select First meeting" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Select Second meeting" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete 2" }));
    await screen.findByRole("button", { name: "Delete 2" });
    expect(confirm).toHaveBeenCalledWith(expect.stringContaining("Permanently delete 2 recordings?"));
    expect(await api.listRecordings()).toHaveLength(3);
    confirm.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: "Delete 2" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: /First meeting/ })).toBeNull());
    expect(screen.queryByRole("button", { name: /Second meeting/ })).toBeNull();
    expect(screen.getByRole("button", { name: /Keep this meeting/ })).toBeTruthy();
    expect(await api.listRecordings()).toHaveLength(1);
  });
});
