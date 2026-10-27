// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { LiveRecording } from "./LiveRecording";
import * as api from "./api";
import type { RecordingSession } from "./types";
import "../tests/selectSetup";

const session: RecordingSession = {
  phase: "recording", recordingId: "current", mode: "online",
  startedAt: "2026-10-27T10:00:00Z", elapsedMs: 65_000, playableMs: 60_000,
  micLevel: 0.5, systemLevel: 0.25, warning: null, error: null,
};
const actions = { onPause: async () => {}, onResume: async () => {}, onStop: async () => {}, onModeChange: async () => {} };

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
  api.resetBrowserMock();
});

describe("live recording", () => {
  it("shows measured levels, freezes history on pause, and clears it for a new session", () => {
    vi.useFakeTimers();
    const { rerender } = render(<LiveRecording session={session} {...actions} />);
    expect(screen.getByLabelText("Recorded duration").textContent).toBe("01:00");
    expect(screen.getByRole("meter", { name: "Microphone level" }).getAttribute("aria-valuenow")).toBe("50");
    expect(screen.getByRole("meter", { name: "Computer audio level" }).getAttribute("aria-valuenow")).toBe("25");
    act(() => vi.advanceTimersByTime(200));
    const wave = screen.getByRole("img", { name: "Microphone recent audio levels" });
    expect(wave.querySelectorAll("rect")).toHaveLength(2);
    expect(wave.querySelector("rect")?.getAttribute("height")).toBe("22");
    rerender(<LiveRecording session={{ ...session, phase: "paused" }} {...actions} />);
    act(() => vi.advanceTimersByTime(1000));
    expect(wave.querySelectorAll("rect")).toHaveLength(2);
    expect(screen.getByRole("meter", { name: "Microphone level" }).getAttribute("aria-valuenow")).toBe("0");
    expect(screen.getByRole("button", { name: "Resume" })).toBeTruthy();
    rerender(<LiveRecording session={{ ...session, recordingId: "next", micLevel: 0 }} {...actions} />);
    expect(wave.querySelectorAll("rect")).toHaveLength(0);
    act(() => vi.advanceTimersByTime(9000));
    expect(wave.querySelectorAll("rect")).toHaveLength(80);
    expect([...wave.querySelectorAll("rect")].every((bar) => bar.getAttribute("height") === "0")).toBe(true);
  });

  it("shows capture warnings and disables controls while starting or saving", () => {
    const { rerender } = render(<LiveRecording session={{ ...session, phase: "starting", mode: "in_person" }} {...actions} />);
    expect(screen.queryByRole("meter", { name: "Computer audio level" })).toBeNull();
    expect((screen.getByRole("button", { name: "Pause" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Microphone" }) as HTMLButtonElement).disabled).toBe(true);
    rerender(<LiveRecording session={{ ...session, phase: "finalizing", warning: "Computer audio is unavailable." }} {...actions} />);
    expect(screen.getByText("Saving recording…")).toBeTruthy();
    expect(screen.getByText("Computer audio is unavailable.")).toBeTruthy();
    expect((screen.getByRole("button", { name: "Stop recording" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "Microphone & computer audio" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("keeps start, pause, resume and stop in the library", async () => {
    await api.completeOnboarding({ launchAtLogin: false });
    const openQuick = vi.spyOn(api, "showQuickPanel");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Start recording" }));
    await screen.findByText("Recording in progress");
    expect(screen.getByRole("region", { name: "Current recording" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Open recorder" }));
    expect(document.activeElement).toBe(screen.getByRole("region", { name: "Current recording" }));
    expect(openQuick).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Microphone" }));
    await waitFor(() => expect(screen.queryByRole("meter", { name: "Computer audio level" })).toBeNull());
    await waitFor(() => expect((screen.getByRole("button", { name: "Pause" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    await screen.findByText("Recording paused");
    await waitFor(() => expect((screen.getByRole("button", { name: "Resume" }) as HTMLButtonElement).disabled).toBe(false));
    vi.spyOn(api, "setRecordingMode").mockRejectedValueOnce(new Error("Computer audio permission is required."));
    fireEvent.click(screen.getByRole("button", { name: "Microphone & computer audio" }));
    await screen.findByText("Computer audio permission is required.");
    expect(screen.getByRole("button", { name: "Microphone" }).getAttribute("aria-pressed")).toBe("true");
    await waitFor(() => expect((screen.getByRole("button", { name: "Microphone & computer audio" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "Microphone & computer audio" }));
    await screen.findByRole("meter", { name: "Computer audio level" });
    expect(screen.getByText("Recording paused")).toBeTruthy();
    await waitFor(() => expect((screen.getByRole("button", { name: "Resume" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "Resume" }));
    await screen.findByText("Recording in progress");
    await waitFor(() => expect((screen.getByRole("button", { name: "Stop recording" }) as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "Stop recording" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Current recording" })).toBeNull());
    expect(await screen.findByText("1 recording")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Start recording" })).toBeTruthy();
  });
});
