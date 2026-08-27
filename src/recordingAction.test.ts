import { describe, expect, it } from "vitest";
import { libraryRecordingAction } from "./recordingAction";

describe("libraryRecordingAction", () => {
  it("starts a recording only from an inactive phase", () => {
    expect(libraryRecordingAction("idle")).toEqual({ kind: "start", label: "Start recording" });
    expect(libraryRecordingAction("ready")).toEqual({ kind: "start", label: "Start recording" });
    expect(libraryRecordingAction("failed")).toEqual({ kind: "start", label: "Start recording" });
  });

  it("opens the recorder when a recording is active", () => {
    expect(libraryRecordingAction("recording")).toEqual({ kind: "open", label: "Open recorder" });
    expect(libraryRecordingAction("paused")).toEqual({ kind: "open", label: "Paused · open recorder" });
  });

  it("disables the action while the recording changes phase", () => {
    expect(libraryRecordingAction("starting")).toEqual({ kind: "pending", label: "Starting…" });
    expect(libraryRecordingAction("finalizing")).toEqual({ kind: "pending", label: "Saving recording…" });
  });
});
