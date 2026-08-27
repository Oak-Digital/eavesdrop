import { useCallback, useEffect, useRef, useState } from "react";
import * as api from "./api";
import type { AppSnapshot, AudioLevels, MeetingCandidate, RecordingSession, StartRecordingInput } from "./types";

export function useAppState() {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [meeting, setMeeting] = useState<MeetingCandidate | null>(null);
  const [error, setError] = useState<string | null>(null);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    let mounted = true;
    let cleanups: Array<() => void> = [];
    api.getSnapshot().then((value) => mounted && setSnapshot(value)).catch((cause) => setError(String(cause)));
    Promise.all([
      api.onSessionChanged((session) => setSnapshot((current) => current ? { ...current, session } : current)),
      api.onAudioLevels((levels: AudioLevels) => setSnapshot((current) => current ? {
        ...current,
        session: { ...current.session, micLevel: levels.mic, systemLevel: levels.system },
      } : current)),
      api.onMeetingCandidate(setMeeting),
      api.onCaptureWarning((warning) => setSnapshot((current) => current ? {
        ...current,
        session: { ...current.session, warning },
      } : current)),
      api.onMeetingEnded(() => setSnapshot((current) => current && ["recording", "paused"].includes(current.session.phase) ? {
        ...current,
        session: { ...current.session, warning: "The meeting appears to have ended. Stop when you are ready." },
      } : current)),
    ]).then((items) => { cleanups = items; });
    return () => {
      mounted = false;
      cleanups.forEach((cleanup) => cleanup());
    };
  }, []);

  useEffect(() => {
    if (!snapshot || !["recording", "paused"].includes(snapshot.session.phase)) {
      window.clearInterval(timer.current);
      return;
    }
    timer.current = window.setInterval(() => {
      setSnapshot((current) => {
        if (!current || current.session.phase !== "recording") return current;
        return {
          ...current,
          session: {
            ...current.session,
            elapsedMs: current.session.elapsedMs + 1000,
            playableMs: current.session.playableMs + 1000,
          },
        };
      });
    }, 1000);
    return () => window.clearInterval(timer.current);
  }, [snapshot?.session.phase]);

  const run = useCallback(async <T,>(operation: () => Promise<T>): Promise<T | undefined> => {
    setError(null);
    try {
      return await operation();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      return undefined;
    }
  }, []);

  const setSession = (session?: RecordingSession) => {
    if (session) setSnapshot((current) => current ? { ...current, session } : current);
  };

  return {
    snapshot,
    meeting,
    error,
    clearError: () => setError(null),
    requestPermissions: async () => {
      const next = await run(api.requestPermissions);
      if (next) setSnapshot(next);
    },
    completeOnboarding: async (launchAtLogin: boolean) => {
      const next = await run(() => api.completeOnboarding({ launchAtLogin }));
      if (next) setSnapshot(next);
    },
    updateSettings: async (settings: Parameters<typeof api.updateSettings>[0]) => {
      const next = await run(() => api.updateSettings(settings));
      if (next) setSnapshot(next);
    },
    installWhisperModel: async (modelId: string) => {
      const next = await run(() => api.installWhisperModel(modelId));
      if (next) setSnapshot(next);
    },
    installSummaryModel: async (modelId: string) => {
      const next = await run(() => api.installSummaryModel(modelId));
      if (next) setSnapshot(next);
    },
    useWhisperModel: async (modelId: string) => {
      const next = await run(() => api.useWhisperModel(modelId));
      if (next) setSnapshot(next);
    },
    useSummaryModel: async (modelId: string) => {
      const next = await run(() => api.useSummaryModel(modelId));
      if (next) setSnapshot(next);
    },
    removeWhisperModel: async (modelId: string) => {
      const next = await run(() => api.removeWhisperModel(modelId));
      if (next) setSnapshot(next);
    },
    removeSummaryModel: async (modelId: string) => {
      const next = await run(() => api.removeSummaryModel(modelId));
      if (next) setSnapshot(next);
    },
    start: async (input: StartRecordingInput) => setSession(await run(() => api.startRecording(input))),
    pause: async () => setSession(await run(api.pauseRecording)),
    resume: async () => setSession(await run(api.resumeRecording)),
    highlight: async () => setSession(await run(api.addHighlight)),
    stop: async () => run(api.stopRecording),
    showQuickPanel: async () => { await run(api.showQuickPanel); },
    dismissMeeting: async () => {
      if (meeting) await run(() => api.dismissMeeting(meeting.id));
      setMeeting(null);
    },
  };
}
