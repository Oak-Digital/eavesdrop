import type { RecordingPhase } from "./types";

export type LibraryRecordingAction = {
  kind: "start" | "open" | "pending";
  label: string;
};

export function libraryRecordingAction(phase: RecordingPhase): LibraryRecordingAction {
  switch (phase) {
    case "starting":
      return { kind: "pending", label: "Starting…" };
    case "recording":
      return { kind: "open", label: "Open recorder" };
    case "paused":
      return { kind: "open", label: "Paused · open recorder" };
    case "finalizing":
      return { kind: "pending", label: "Saving recording…" };
    default:
      return { kind: "start", label: "Start recording" };
  }
}
