// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import App from "./App";
import * as api from "./api";
import "../tests/selectSetup";

afterEach(() => { cleanup(); api.resetBrowserMock(); });

async function openSettings() {
  await api.completeOnboarding({ launchAtLogin: false });
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
  await screen.findByRole("tab", { name: "General" });
}

describe("Settings navigation", () => {
  it("shows only the chosen section and keeps permissions with recording", async () => {
    await openSettings();
    expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
    expect(screen.getByRole("combobox", { name: "Theme" })).toBeTruthy();
    expect(screen.queryByRole("combobox", { name: "Whisper model" })).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "Recording" }));
    expect(screen.getByRole("combobox", { name: "Microphone" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Check permissions" })).toBeTruthy();
    expect(screen.queryByRole("combobox", { name: "Theme" })).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "Transcription" }));
    expect(screen.getByRole("combobox", { name: "Whisper model" })).toBeTruthy();
    fireEvent.click(screen.getByRole("tab", { name: "About" }));
    expect(screen.getByRole("button", { name: "Check for updates" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Export diagnostics" })).toBeTruthy();
  });

  it("supports arrow keys, wrapping, Home and End with a single tab stop", async () => {
    await openSettings();
    const general = screen.getByRole("tab", { name: "General" });
    const about = screen.getByRole("tab", { name: "About" });
    fireEvent.keyDown(general, { key: "ArrowLeft" });
    expect(document.activeElement).toBe(about);
    expect(about.getAttribute("aria-selected")).toBe("true");
    fireEvent.keyDown(about, { key: "ArrowRight" });
    expect(document.activeElement).toBe(general);
    fireEvent.keyDown(general, { key: "End" });
    expect(document.activeElement).toBe(about);
    fireEvent.keyDown(about, { key: "Home" });
    expect(document.activeElement).toBe(general);
    expect(screen.getAllByRole("tab").filter((tab) => tab.tabIndex === 0)).toHaveLength(1);
  });

  it("preserves unsaved summary drafts and does not change processing when opening cloud setup", async () => {
    await api.updateSettings({ summaryTemplate: "custom", summaryPrompt: "Saved instructions" });
    await openSettings();
    fireEvent.click(screen.getByRole("tab", { name: "Summaries" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Custom summary prompt" }), { target: { value: "Unsaved draft" } });
    fireEvent.click(screen.getByText("Cloud provider", { selector: "summary" }));
    fireEvent.change(screen.getByLabelText("OpenAI API key"), { target: { value: "test-draft-not-a-real-key" } });
    fireEvent.click(screen.getByRole("tab", { name: "General" }));
    expect(screen.queryByRole("textbox", { name: "Custom summary prompt" })).toBeNull();
    fireEvent.click(screen.getByRole("tab", { name: "Summaries" }));
    expect((screen.getByRole("textbox", { name: "Custom summary prompt" }) as HTMLTextAreaElement).value).toBe("Unsaved draft");
    expect((screen.getByLabelText("OpenAI API key") as HTMLInputElement).value).toBe("test-draft-not-a-real-key");
    const snapshot = await api.getSnapshot();
    expect(snapshot.settings.summarySource).toBe("local");
    expect(snapshot.settings.summaryPrompt).toBe("Saved instructions");
  });
});
