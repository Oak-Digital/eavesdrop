// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import * as api from "./api";
import "../tests/selectSetup";

beforeEach(api.resetBrowserMock);
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

async function openLibrary() {
  await api.completeOnboarding({ launchAtLogin: false });
  for (const title of ["Product planning", "Product review", "Customer interview"]) {
    await api.startRecording({ mode: "online" });
    const recording = await api.stopRecording();
    await api.renameRecording(recording.id, title);
  }
  render(<App />);
  await screen.findByRole("checkbox", { name: "Select Product planning" });
}

const checkbox = (name: string) => screen.getByRole("checkbox", { name }) as HTMLInputElement;

describe("recording checkboxes", () => {
  it("selects immediately and keeps select-all checked, partial, and empty states in sync", async () => {
    await openLibrary();
    expect(screen.queryByRole("button", { name: "Select" })).toBeNull();
    expect(screen.getAllByRole("checkbox")).toHaveLength(4);
    const all = checkbox("Select all visible recordings");
    expect(all.checked).toBe(false);
    expect(all.indeterminate).toBe(false);

    fireEvent.click(checkbox("Select Product planning"));
    expect(screen.queryByLabelText("Recording title")).toBeNull();
    expect(screen.getByText("1 selected")).toBeTruthy();
    expect(all.indeterminate).toBe(true);
    fireEvent.click(all);
    expect(screen.getByText("3 selected")).toBeTruthy();
    expect(screen.getAllByRole("checkbox").every((input) => (input as HTMLInputElement).checked)).toBe(true);
    expect(all.indeterminate).toBe(false);
    fireEvent.click(all);
    expect(screen.getAllByRole("checkbox").every((input) => !(input as HTMLInputElement).checked)).toBe(true);
    expect(screen.getByRole("button", { name: "Start recording" })).toBeTruthy();
    fireEvent.click(checkbox("Select Product review"));
    fireEvent.click(screen.getByRole("button", { name: "Clear selection" }));
    expect(all.indeterminate).toBe(false);
    expect(checkbox("Select Product review").checked).toBe(false);
  });

  it("selects only search results and preserves selections outside the current filter", async () => {
    await openLibrary();
    fireEvent.click(checkbox("Select Customer interview"));
    fireEvent.change(screen.getByPlaceholderText("Search recordings"), { target: { value: "Product" } });
    const all = checkbox("Select all visible recordings");
    expect(all.checked).toBe(false);
    expect(all.indeterminate).toBe(false);
    fireEvent.click(all);
    expect(screen.getByText("3 selected")).toBeTruthy();
    fireEvent.click(all);
    expect(screen.getByText("1 selected")).toBeTruthy();
    fireEvent.change(screen.getByPlaceholderText("Search recordings"), { target: { value: "No matches" } });
    expect(all.disabled).toBe(true);
    fireEvent.change(screen.getByPlaceholderText("Search recordings"), { target: { value: "" } });
    expect(checkbox("Select Customer interview").checked).toBe(true);
    expect(checkbox("Select Product planning").checked).toBe(false);
    expect(all.indeterminate).toBe(true);
  });

  it("opens a recording independently from checkbox selection", async () => {
    await openLibrary();
    fireEvent.click(checkbox("Select Product planning"));
    fireEvent.click(screen.getByRole("button", { name: /Product review/ }));
    expect((await screen.findByLabelText("Recording title") as HTMLInputElement).value).toBe("Product review");
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(checkbox("Select Product planning").checked).toBe(true);
    expect(checkbox("Select Product review").checked).toBe(false);
  });

  it("locks the selection while deletion confirmation is pending", async () => {
    let resolveConfirmation!: (value: boolean) => void;
    vi.spyOn(api, "confirmRecordingDeletion").mockReturnValue(new Promise((resolve) => { resolveConfirmation = resolve; }));
    await openLibrary();
    fireEvent.click(checkbox("Select Product planning"));
    fireEvent.click(screen.getByRole("button", { name: "Delete 1" }));
    expect(screen.getAllByRole("checkbox").every((input) => (input as HTMLInputElement).disabled)).toBe(true);
    resolveConfirmation(false);
    await waitFor(() => expect(checkbox("Select Product planning").disabled).toBe(false));
    expect(checkbox("Select Product planning").checked).toBe(true);
    expect(await api.listRecordings()).toHaveLength(3);
  });
});
