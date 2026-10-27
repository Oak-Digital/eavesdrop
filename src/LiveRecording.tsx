import { useEffect, useRef, useState } from "react";
import { formatDuration } from "./format";
import { ComputerIcon, MicIcon, PauseIcon, PlayIcon, StopIcon } from "./icons";
import type { CaptureMode, RecordingSession } from "./types";

const SAMPLE_COUNT = 80;
type Sample = { mic: number; system: number };
const level = (value: number) => Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : 0;

export function LiveRecording({ session, onPause, onResume, onStop, onModeChange }: {
  session: RecordingSession;
  onPause: () => Promise<unknown>;
  onResume: () => Promise<unknown>;
  onStop: () => Promise<unknown>;
  onModeChange: (mode: CaptureMode) => Promise<unknown>;
}) {
  const latest = useRef(session);
  const [samples, setSamples] = useState<Sample[]>([]);
  const [pending, setPending] = useState(false);
  const actionPending = useRef(false);
  const paused = session.phase === "paused";
  const recording = session.phase === "recording";
  const busy = pending || (!recording && !paused);
  const status = session.phase === "starting" ? "Starting recording…"
    : session.phase === "finalizing" ? "Saving recording…"
    : paused ? "Recording paused" : "Recording in progress";

  useEffect(() => { latest.current = session; }, [session]);
  useEffect(() => { setSamples([]); }, [session.recordingId]);
  useEffect(() => {
    setSamples((history) => history.map((sample) => ({ ...sample, system: 0 })));
  }, [session.mode]);
  useEffect(() => {
    if (!recording) return;
    const timer = window.setInterval(() => {
      const current = latest.current;
      if (current.phase !== "recording") return;
      setSamples((history) => [...history.slice(-(SAMPLE_COUNT - 1)), {
        mic: level(current.micLevel), system: current.mode === "online" ? level(current.systemLevel) : 0,
      }]);
    }, 100);
    return () => window.clearInterval(timer);
  }, [recording, session.recordingId]);

  const run = async (action: () => Promise<unknown>) => {
    if (actionPending.current) return;
    actionPending.current = true;
    setPending(true);
    try { await action(); } finally {
      actionPending.current = false;
      setPending(false);
    }
  };

  return (
    <section className="live-recording" id="live-recording" aria-label="Current recording" tabIndex={-1}>
      <div className="live-recording-heading">
        <div className="live-recording-status">
          <h2 role="status"><span className={recording ? "record-dot" : "record-dot paused"} aria-hidden="true" />{status}</h2>
          <time className="live-recording-time" aria-label="Recorded duration">{formatDuration(session.playableMs)}</time>
        </div>
        <div className="live-recording-actions">
          <button className="button secondary" disabled={busy} onClick={() => void run(paused ? onResume : onPause)}>
            {paused ? <PlayIcon /> : <PauseIcon />}{paused ? "Resume" : "Pause"}
          </button>
          <button className="button danger" disabled={busy} onClick={() => void run(onStop)}><StopIcon />Stop recording</button>
        </div>
      </div>
      <div className="recording-source-toggle" role="group" aria-label="Recording sources" aria-busy={pending}>
        <button aria-pressed={session.mode === "in_person"} disabled={busy} onClick={() => void run(() => onModeChange("in_person"))}>Microphone</button>
        <button aria-pressed={session.mode === "online"} disabled={busy} onClick={() => void run(() => onModeChange("online"))}>Microphone &amp; computer audio</button>
      </div>
      <div className="live-recording-tracks" title="Audio levels over the last 8 seconds">
        <LevelHistory label="Microphone" source="mic" samples={samples} value={recording ? session.micLevel : 0} />
        {session.mode !== "in_person" && <LevelHistory label="Computer audio" source="system" samples={samples} value={recording ? session.systemLevel : 0} />}
      </div>
      {session.warning && <div className="warning-row" role="status">{session.warning}</div>}
      {session.error && <div className="warning-row" role="alert">{session.error}</div>}
    </section>
  );
}

function LevelHistory({ label, source, samples, value }: {
  label: string; source: keyof Sample; samples: Sample[]; value: number;
}) {
  const percentage = Math.round(level(value) * 100);
  return (
    <div className="live-recording-track">
      <div className="live-recording-source">
        {source === "mic" ? <MicIcon /> : <ComputerIcon />}<span>{label}</span>
      </div>
      <svg className="live-recording-wave" viewBox="0 0 640 48" preserveAspectRatio="none" role="img" aria-label={`${label} recent audio levels`}>
        <line x1="0" x2="640" y1="24" y2="24" className="live-recording-baseline" />
        {samples.map((sample, index) => {
          const height = sample[source] * 44;
          return <rect key={index} x={(SAMPLE_COUNT - samples.length + index) * 8 + 2} y={24 - height / 2} width="4" height={height} rx="1" />;
        })}
      </svg>
      <span className="live-recording-level" role="meter" aria-label={`${label} level`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={percentage}>{percentage}%</span>
    </div>
  );
}
