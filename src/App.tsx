import { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "./api";
import { formatDate, formatDuration, formatSize, transcriptionPercentage } from "./format";
import {
  ChevronLeftIcon,
  ComputerIcon,
  ExportIcon,
  MarkerIcon,
  MicIcon,
  MoreIcon,
  PauseIcon,
  PlayIcon,
  RecordingsIcon,
  SearchIcon,
  SettingsIcon,
  StopIcon,
  TrashIcon,
} from "./icons";
import type { AppSnapshot, CaptureMode, ModelDownloadStatus, Recording, SummarizationStage, SummaryModelInfo, TranscriptionStage, WhisperModelInfo } from "./types";
import { libraryRecordingAction } from "./recordingAction";
import { applyTheme } from "./theme";
import { useAppState } from "./useAppState";
import { useUpdater } from "./useUpdater";

type LibraryPage = "recordings" | "deleted" | "settings";
const isMac = /Macintosh|Mac OS X/.test(navigator.userAgent);
const DEFAULT_SUMMARY_PROMPT = "Summarize the meeting clearly and concisely. Focus on the topics discussed, decisions reached, and tasks assigned, including who owns them.";

function beginWindowDrag(event: React.MouseEvent<HTMLElement>) {
  if (event.button !== 0 || !("__TAURI_INTERNALS__" in window)) return;
  const target = event.target as HTMLElement;
  if (target.closest("button, input, select, a, [role='button']")) return;
  event.preventDefault();
  void getCurrentWindow().startDragging();
}

export default function App() {
  const app = useAppState();
  const surface = new URLSearchParams(window.location.search).get("surface") ?? "library";
  const updater = useUpdater(surface !== "quick");

  useEffect(() => {
    if (app.snapshot) applyTheme(app.snapshot.settings.theme);
  }, [app.snapshot?.settings.theme]);

  if (!app.snapshot) return <div className="loading">Opening Eavesdrop…</div>;
  if (surface === "quick") return <QuickPanel app={app} />;
  return <LibraryApp app={app} updater={updater} />;
}

type AppController = ReturnType<typeof useAppState>;
type AppUpdater = ReturnType<typeof useUpdater>;

function QuickPanel({ app }: { app: AppController }) {
  const { snapshot, meeting } = app;
  if (!snapshot) return null;
  const active = ["recording", "paused", "starting", "finalizing"].includes(snapshot.session.phase);

  return (
    <main className="quick-panel">
      <div className="quick-drag-region" onMouseDown={beginWindowDrag}>
        <div className="wordmark"><BrandMark active={active} /> Eavesdrop</div>
        <button className="icon-button compact" onClick={() => api.hideQuickPanel()} aria-label="Close recorder">×</button>
      </div>

      {app.error && <InlineError message={app.error} onClose={app.clearError} />}
      {meeting && !active ? (
        <MeetingPrompt app={app} />
      ) : active ? (
        <ActiveRecorder app={app} />
      ) : (
        <IdleRecorder snapshot={snapshot} onStart={app.start} />
      )}

      <button className="quick-footer" onClick={() => api.openLibrary()}>
        Open recordings
        <span aria-hidden="true">⌘O</span>
      </button>
    </main>
  );
}

function IdleRecorder({ snapshot, onStart }: { snapshot: AppSnapshot; onStart: AppController["start"] }) {
  const micReady = snapshot.permissions.microphone === "granted";
  return (
    <section className="quick-content">
      <div className="source-status">
        <span className={micReady ? "status-ok" : "status-warn"} />
        {micReady ? snapshot.devices.find((item) => item.isDefault)?.name ?? "Microphone ready" : "Microphone permission needed"}
      </div>
      <div className="recording-choices">
        <button className="recording-choice" onClick={() => onStart({ mode: "in_person" })}>
          <MicIcon />
          <span><strong>In-person meeting</strong><em>Microphone</em></span>
        </button>
        <button className="recording-choice primary-choice" onClick={() => onStart({ mode: "online" })}>
          <ComputerIcon />
          <span><strong>Online meeting</strong><em>Microphone + computer</em></span>
        </button>
      </div>
      <p className="consent-note">Always let participants know before you record.</p>
    </section>
  );
}

function MeetingPrompt({ app }: { app: AppController }) {
  const meeting = app.meeting!;
  return (
    <section className="quick-content meeting-prompt">
      <div className="meeting-app-icon">{meeting.app === "meet" ? "G" : meeting.app === "teams" ? "T" : "Z"}</div>
      <div>
        <h2>Meeting started</h2>
        <p>{meeting.displayName} appears to be in a call.</p>
      </div>
      <div className="button-row">
        <button className="button secondary" onClick={app.dismissMeeting}>Not now</button>
        <button className="button primary" onClick={() => app.start({ mode: "online", detectedApp: meeting.displayName })}>Record</button>
      </div>
    </section>
  );
}

function ActiveRecorder({ app }: { app: AppController }) {
  const session = app.snapshot!.session;
  const paused = session.phase === "paused";
  const busy = session.phase === "starting" || session.phase === "finalizing";
  return (
    <section className="quick-content active-recorder">
      <div className="recording-clock">
        <span className={paused ? "record-dot paused" : "record-dot"} />
        <time>{formatDuration(session.playableMs)}</time>
        <span>{paused ? "Paused" : session.mode === "online" ? "Online meeting" : "In-person meeting"}</span>
      </div>

      <div className="level-group">
        <AudioMeter icon={<MicIcon />} label="Microphone" value={session.micLevel} />
        {session.mode === "online" && <AudioMeter icon={<ComputerIcon />} label="Computer" value={session.systemLevel} />}
      </div>
      {session.warning && <div className="warning-row">{session.warning}</div>}

      <div className="recording-controls">
        <button className="control-button" disabled={busy} onClick={paused ? app.resume : app.pause}>
          {paused ? <PlayIcon /> : <PauseIcon />}
          <span>{paused ? "Resume" : "Pause"}</span>
        </button>
        <button className="control-button" disabled={busy || paused} onClick={app.highlight}>
          <MarkerIcon />
          <span>Highlight</span>
        </button>
        <button className="control-button stop-control" disabled={busy} onClick={async () => {
          const recording = await app.stop();
          if (recording) await api.openLibrary(recording.id);
        }}>
          <StopIcon />
          <span>Stop</span>
        </button>
      </div>
    </section>
  );
}

function AudioMeter({ icon, label, value }: { icon: React.ReactNode; label: string; value: number }) {
  const percentage = value < 0.005 ? 0 : Math.min(100, Math.max(3, value * 100));
  return (
    <div className="audio-meter">
      <span className="meter-label">{icon}{label}</span>
      <span
        className="meter-track"
        role="meter"
        aria-label={`${label} level`}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(percentage)}
      >
        <span style={{ width: `${percentage}%` }} />
      </span>
    </div>
  );
}

function LibraryApp({ app, updater }: { app: AppController; updater: AppUpdater }) {
  const [page, setPage] = useState<LibraryPage>("recordings");
  const pageRef = useRef<LibraryPage>("recordings");
  const [recordings, setRecordings] = useState<Recording[]>([]);
  const [selected, setSelected] = useState<Recording | null>(null);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(true);
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set());
  const [bulkBusy, setBulkBusy] = useState(false);
  const [bulkError, setBulkError] = useState<string | null>(null);
  const [startingFromLibrary, setStartingFromLibrary] = useState(false);
  const [activity, setActivity] = useState<ActivityState | null>(null);
  const activityRef = useRef<ActivityState | null>(null);
  const models = useModelManager(app);

  const reload = async () => {
    setLoading(true);
    const items = await api.listRecordings(page === "deleted");
    setRecordings(items);
    setLoading(false);
  };

  useEffect(() => { void reload(); }, [page]);

  useEffect(() => {
    pageRef.current = page;
    setSelectionMode(false);
    setSelectedIds(new Set());
    setBulkError(null);
  }, [page]);

  useEffect(() => {
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    const keep = (unlisten: () => void) => {
      if (disposed) unlisten(); else unlisteners.push(unlisten);
    };

    void api.onRecordingFinalized((recording) => {
      if (pageRef.current !== "recordings") return;
      setRecordings((items) => [recording, ...items.filter((item) => item.id !== recording.id)]);
      setSelected((current) => current?.id === recording.id ? recording : current);
      setLoading(false);
    }).then(keep);

    // Ignore stragglers for a run the detail view already finished or cancelled.
    const advance = (kind: ActivityKind, recordingId: string, label: string, progress: number) =>
      setActivity((current) => {
        if (!current || current.kind !== kind || current.recordingId !== recordingId) return current;
        const next = { ...current, label, progress };
        activityRef.current = next;
        return next;
      });

    void api.onTranscriptionProgress((progress) => {
      advance("transcription", progress.recordingId, transcriptionLabel(progress.stage), progress.progress);
    }).then(keep);

    void api.onSummarizationProgress((progress) => {
      advance("summary", progress.recordingId, summarizationLabel(progress.stage), progress.progress);
    }).then(keep);

    void api.onOpenRecording(async (recordingId) => {
      const items = await api.listRecordings(false);
      if (disposed) return;
      pageRef.current = "recordings";
      setPage("recordings");
      setRecordings(items);
      setSelected(items.find((item) => item.id === recordingId) ?? null);
      setLoading(false);
    }).then(keep);

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  const visible = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase();
    return normalized ? recordings.filter((item) => item.title.toLocaleLowerCase().includes(normalized)) : recordings;
  }, [query, recordings]);

  const toggleSelection = (recordingId: string) => {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(recordingId)) next.delete(recordingId); else next.add(recordingId);
      return next;
    });
  };

  const allVisibleSelected = visible.length > 0 && visible.every((recording) => selectedIds.has(recording.id));
  const toggleAllVisible = () => {
    setSelectedIds((current) => {
      const next = new Set(current);
      visible.forEach((recording) => {
        if (allVisibleSelected) next.delete(recording.id); else next.add(recording.id);
      });
      return next;
    });
  };

  const exitSelection = () => {
    setSelectionMode(false);
    setSelectedIds(new Set());
    setBulkError(null);
  };

  const applyBulkAction = async () => {
    const ids = [...selectedIds];
    if (!ids.length) return;
    if (page === "recordings" && !window.confirm(`Move ${ids.length} ${ids.length === 1 ? "recording" : "recordings"} to Recently Deleted?`)) return;
    setBulkBusy(true);
    setBulkError(null);
    try {
      if (page === "deleted") await api.restoreRecordings(ids); else await api.deleteRecordings(ids);
      await reload();
      exitSelection();
    } catch (cause) {
      setBulkError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBulkBusy(false);
    }
  };

  const recordingAction = startingFromLibrary
    ? { kind: "pending" as const, label: "Starting…" }
    : libraryRecordingAction(app.snapshot?.session.phase ?? "idle");

  const handleRecordingAction = async () => {
    if (recordingAction.kind === "open") {
      await app.showQuickPanel();
      return;
    }
    if (recordingAction.kind !== "start") return;
    setStartingFromLibrary(true);
    try {
      await app.start({ mode: "online" });
    } finally {
      setStartingFromLibrary(false);
    }
  };

  const beginActivity = (item: Recording, kind: ActivityKind, label: string) => {
    if (activityRef.current) return false;
    const next = { kind, recordingId: item.id, title: item.title, label, progress: 0 };
    activityRef.current = next;
    setActivity(next);
    return true;
  };

  const endActivity = (recordingId: string, kind: ActivityKind) => {
    const current = activityRef.current;
    if (!current || current.kind !== kind || current.recordingId !== recordingId) return;
    activityRef.current = null;
    setActivity((value) => value?.kind === kind && value.recordingId === recordingId ? null : value);
  };

  if (!app.snapshot) return null;
  if (!app.snapshot.settings.onboardingCompleted) {
    return <Onboarding app={app} />;
  }

  return (
    <main className={isMac ? "library-shell mac-window" : "library-shell"}>
      {isMac && <div className="window-drag-bar" onMouseDown={beginWindowDrag} aria-hidden="true" />}
      <aside className="sidebar">
        <div className="sidebar-brand" onMouseDown={beginWindowDrag}><BrandMark active={app.snapshot.session.phase === "recording"} /> Eavesdrop</div>
        <nav aria-label="Library">
          <button className={page === "recordings" ? "active" : ""} onClick={() => { setPage("recordings"); setSelected(null); }}><RecordingsIcon /> Recordings</button>
          <button className={page === "deleted" ? "active" : ""} onClick={() => { setPage("deleted"); setSelected(null); }}><TrashIcon /> Recently deleted</button>
        </nav>
        <div className="sidebar-bottom">
          {models.download && <ModelDownloadProgressCard models={models} />}
          {activity && <ActivityProgressCard activity={activity} />}
          <button className={page === "settings" ? "active" : ""} onClick={() => { setPage("settings"); setSelected(null); }}><SettingsIcon /> Settings</button>
          <div className="privacy-line">Audio stays on this computer</div>
        </div>
      </aside>

      <section className="library-content">
        <UpdateBanner app={app} updater={updater} />
        {app.error && <InlineError message={app.error} onClose={app.clearError} />}
        {bulkError && <InlineError message={bulkError} onClose={() => setBulkError(null)} />}
        {selected ? (
          <RecordingDetail
            recording={selected}
            deleted={page === "deleted"}
            whisperModelPath={app.snapshot.settings.whisperModelPath}
            summaryModelPath={app.snapshot.settings.summaryModelPath}
            activity={activity}
            onChooseWhisperModel={async () => {
              const path = await api.selectWhisperModel();
              if (path) await app.updateSettings({ whisperModelPath: path });
            }}
            onChooseSummaryModel={async () => {
              const path = await api.selectSummaryModel();
              if (path) await app.updateSettings({ summaryModelPath: path });
            }}
            onBack={() => setSelected(null)}
            onJobStart={beginActivity}
            onJobEnd={endActivity}
            onChanged={(recording) => {
              setSelected(recording);
              setRecordings((items) => items.map((item) => item.id === recording.id ? recording : item));
            }}
            onRemoved={async () => { setSelected(null); await reload(); }}
          />
        ) : page === "settings" ? (
          <SettingsPage app={app} updater={updater} models={models} />
        ) : (
          <>
            <header className="content-header">
              <div><h1>{page === "deleted" ? "Recently deleted" : "Recordings"}</h1><p>{page === "deleted" ? "Items are removed permanently after seven days." : "Your local meeting library."}</p></div>
              <div className="header-actions">
                {selectionMode ? (
                  <>
                    <button className="button secondary" disabled={bulkBusy} onClick={exitSelection}>Cancel</button>
                    <button className={page === "deleted" ? "button primary" : "button danger"} disabled={bulkBusy || selectedIds.size === 0} onClick={applyBulkAction}>
                      {page === "deleted" ? `Restore ${selectedIds.size || ""}` : <><TrashIcon /> Move {selectedIds.size || ""} to Deleted</>}
                    </button>
                  </>
                ) : (
                  <>
                    {recordings.length > 0 && <button className="button secondary" onClick={() => { setSelected(null); setSelectionMode(true); }}>Select</button>}
                    {page === "recordings" && (
                      <button className="button primary" disabled={recordingAction.kind === "pending"} onClick={handleRecordingAction}>
                        {(recordingAction.kind === "start" || app.snapshot.session.phase === "recording") && <span className="button-record-dot" />}
                        {app.snapshot.session.phase === "paused" && <PauseIcon />}
                        {recordingAction.label}
                      </button>
                    )}
                  </>
                )}
              </div>
            </header>
            <div className="list-toolbar">
              <label className="search-field"><SearchIcon /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search recordings" /></label>
              <div className="toolbar-summary">
                {selectionMode && visible.length > 0 && <button className="toolbar-select" onClick={toggleAllVisible}>{allVisibleSelected ? "Deselect visible" : "Select all visible"}</button>}
                <span>{selectionMode ? `${selectedIds.size} selected` : `${visible.length} ${visible.length === 1 ? "recording" : "recordings"}`}</span>
              </div>
            </div>
            <RecordingList items={visible} loading={loading} deleted={page === "deleted"} selectionMode={selectionMode} selectedIds={selectedIds} onSelect={setSelected} onToggleSelection={toggleSelection} />
          </>
        )}
      </section>
    </main>
  );
}

// One long-running local job — a transcript or a summary — shown in the sidebar.
export type ActivityKind = "transcription" | "summary";
export type ActivityState = { kind: ActivityKind; recordingId: string; title: string; label: string; progress: number };

function useModelManager(app: AppController) {
  const [whisperModels, setWhisperModels] = useState<WhisperModelInfo[]>([]);
  const [summaryModels, setSummaryModels] = useState<SummaryModelInfo[]>([]);
  const [download, setDownload] = useState<ModelDownloadStatus | null>(null);

  const reload = async () => {
    const [whisper, summary] = await Promise.all([api.listWhisperModels(), api.listSummaryModels()]);
    setWhisperModels(whisper);
    setSummaryModels(summary);
  };

  useEffect(() => {
    let disposed = false;
    const cleanups: Array<() => void> = [];
    const keep = (cleanup: () => void) => { if (disposed) cleanup(); else cleanups.push(cleanup); };
    void api.onModelDownloadStatus((status) => {
      if (disposed) return;
      if (status.active) {
        setDownload(status);
      } else {
        setDownload(null);
        void reload();
      }
    }).then(keep);
    void Promise.all([api.listWhisperModels(), api.listSummaryModels(), api.getModelDownloadStatus()]).then(([whisper, summary, status]) => {
      if (!disposed) {
        setWhisperModels(whisper);
        setSummaryModels(summary);
        if (status?.active) setDownload(status);
      }
    });
    return () => { disposed = true; cleanups.forEach((cleanup) => cleanup()); };
  }, []);

  const reconcile = async () => {
    try {
      await reload();
    } finally {
      const status = await api.getModelDownloadStatus().catch(() => null);
      setDownload(status?.active ? status : null);
    }
  };

  return {
    whisperModels,
    summaryModels,
    download,
    installWhisper: async (modelId: string) => {
      setDownload({ kind: "whisper", modelId, downloadedBytes: 0, totalBytes: 0, active: true });
      try { await app.installWhisperModel(modelId); } finally { await reconcile(); }
    },
    installSummary: async (modelId: string) => {
      setDownload({ kind: "summary", modelId, downloadedBytes: 0, totalBytes: 0, active: true });
      try { await app.installSummaryModel(modelId); } finally { await reconcile(); }
    },
    useWhisper: async (modelId: string) => {
      await app.useWhisperModel(modelId);
      await reload();
    },
    useSummary: async (modelId: string) => {
      await app.useSummaryModel(modelId);
      await reload();
    },
    removeWhisper: async (modelId: string) => {
      await app.removeWhisperModel(modelId);
      await reload();
    },
    removeSummary: async (modelId: string) => {
      await app.removeSummaryModel(modelId);
      await reload();
    },
  };
}

type ModelManager = ReturnType<typeof useModelManager>;

function ModelDownloadProgressCard({ models }: { models: ModelManager }) {
  const download = models.download!;
  const catalog = download.kind === "whisper" ? models.whisperModels : models.summaryModels;
  const model = catalog.find((item) => item.id === download.modelId);
  const percent = download.totalBytes
    ? Math.min(100, Math.round(download.downloadedBytes / download.totalBytes * 100))
    : 0;
  return (
    <div className="sidebar-progress" role="status" aria-live="polite">
      <div className="sidebar-progress-head">
        <span className="sidebar-progress-label">Downloading model</span>
        <span className="sidebar-progress-value">{percent}%</span>
      </div>
      <div className="sidebar-progress-track" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent}>
        <div className="sidebar-progress-fill" style={{ width: `${percent}%` }} />
      </div>
      <div className="sidebar-progress-title" title={model?.name}>{model?.name ?? download.modelId}</div>
    </div>
  );
}

function transcriptionLabel(stage: TranscriptionStage) {
  return stage === "decoding" ? "Preparing audio" : "Transcribing";
}

function summarizationLabel(stage: SummarizationStage) {
  if (stage === "loading") return "Loading summary model";
  return stage === "analyzing" ? "Reading the meeting" : "Writing the summary";
}

function ActivityProgressCard({ activity }: { activity: ActivityState }) {
  const percent = transcriptionPercentage(activity.progress);
  const label = activity.label;
  return (
    <div className="sidebar-progress" role="status" aria-live="polite">
      <div className="sidebar-progress-head">
        <span className="sidebar-progress-label">{label}</span>
        <span className="sidebar-progress-value">{percent}%</span>
      </div>
      <div
        className="sidebar-progress-track"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent}
        aria-label={`${label} ${activity.title}`}
      >
        <div className="sidebar-progress-fill" style={{ width: `${percent}%` }} />
      </div>
      <div className="sidebar-progress-title" title={activity.title}>{activity.title}</div>
    </div>
  );
}

function RecordingList({ items, loading, deleted, selectionMode, selectedIds, onSelect, onToggleSelection }: { items: Recording[]; loading: boolean; deleted: boolean; selectionMode: boolean; selectedIds: Set<string>; onSelect: (item: Recording) => void; onToggleSelection: (id: string) => void }) {
  if (loading) return <div className="empty-state">Loading recordings…</div>;
  if (items.length === 0) return <div className="empty-state"><RecordingsIcon /><strong>{deleted ? "Nothing recently deleted" : "No recordings yet"}</strong><p>{deleted ? "Deleted recordings will appear here for seven days." : "Start from the menu bar or the button above."}</p></div>;
  return (
    <div className="recording-list">
      {items.map((item) => {
        const selected = selectedIds.has(item.id);
        return (
        <button className={selected ? "recording-row selected" : "recording-row"} key={item.id} aria-pressed={selectionMode ? selected : undefined} onClick={() => selectionMode ? onToggleSelection(item.id) : onSelect(item)}>
          {selectionMode ? <span className="selection-box" aria-hidden="true">{selected ? "✓" : ""}</span> : <span className="play-circle"><PlayIcon /></span>}
          <span className="recording-main"><strong>{item.title}</strong><em>{formatDate(item.startedAt)} · {item.mode === "online" ? "Online" : "In-person"}</em></span>
          <time>{formatDuration(item.playableDurationMs)}</time>
          <span className="recording-size">{formatSize(item.sizeBytes)}</span>
          {!selectionMode && <MoreIcon />}
        </button>
      )})}
    </div>
  );
}

export function RecordingDetail({ recording, deleted, whisperModelPath, summaryModelPath, activity, onChooseWhisperModel, onChooseSummaryModel, onBack, onJobStart, onJobEnd, onChanged, onRemoved }: { recording: Recording; deleted: boolean; whisperModelPath: string | null; summaryModelPath: string | null; activity: ActivityState | null; onChooseWhisperModel: () => Promise<void>; onChooseSummaryModel: () => Promise<void>; onBack: () => void; onJobStart: (item: Recording, kind: ActivityKind, label: string) => boolean; onJobEnd: (id: string, kind: ActivityKind) => void; onChanged: (item: Recording) => void; onRemoved: () => void }) {
  const [title, setTitle] = useState(recording.title);
  const [busy, setBusy] = useState(false);
  const [titleError, setTitleError] = useState<string | null>(null);
  const [transcriptionError, setTranscriptionError] = useState<string | null>(null);
  const [summaryError, setSummaryError] = useState<string | null>(null);
  const transcribing = activity?.kind === "transcription" && activity.recordingId === recording.id;
  const summarizing = activity?.kind === "summary" && activity.recordingId === recording.id;
  const processing = activity !== null;
  useEffect(() => {
    setTitle(recording.title);
    setTitleError(null);
  }, [recording.id, recording.title]);

  const saveTitle = async (next = title) => {
    if (busy) return;
    const trimmed = next.trim();
    setTitleError(null);
    if (!trimmed || trimmed === recording.title) {
      setTitle(recording.title);
      return;
    }
    setBusy(true);
    try {
      const renamed = await api.renameRecording(recording.id, trimmed);
      setTitle(renamed.title);
      onChanged(renamed);
    } catch (cause) {
      setTitle(recording.title);
      setTitleError(cause instanceof Error ? cause.message : "The title could not be saved.");
    } finally {
      setBusy(false);
    }
  };

  const useSuggestedTitle = async (suggested: string) => {
    setTitle(suggested);
    await saveTitle(suggested);
  };
  return (
    <div className="recording-detail">
      <header className="detail-header">
        <button className="icon-button" onClick={onBack} aria-label="Back"><ChevronLeftIcon /></button>
        <div className="title-editor">
          <input
            value={title}
            readOnly={busy}
            aria-busy={busy}
            aria-invalid={titleError ? true : undefined}
            aria-describedby={titleError ? "recording-title-error" : undefined}
            onChange={(event) => { setTitle(event.target.value); setTitleError(null); }}
            onBlur={() => { void saveTitle(); }}
            onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }}
            aria-label="Recording title"
          />
          <span>{formatDate(recording.startedAt)} · {formatDuration(recording.playableDurationMs)} · {formatSize(recording.sizeBytes)}</span>
          {titleError && <span className="title-error" id="recording-title-error" role="alert">{titleError}</span>}
        </div>
        {!deleted && <button className="button primary" onClick={() => api.exportRecording(recording)}><ExportIcon /> Export M4A</button>}
        {deleted && <button className="button primary" onClick={async () => { await api.restoreRecording(recording.id); onRemoved(); }}>Restore</button>}
      </header>

      <EncryptedAudioPlayer recording={recording} />

      <section className="detail-section summary-section">
        <div className="section-heading">
          <div><h2>Summary</h2>{recording.summary && <span>{recording.summary.model}</span>}</div>
          {!deleted && (summaryModelPath ? (
            <button className="button secondary compact-button" disabled={processing || !recording.transcript} onClick={async () => {
              if (!onJobStart(recording, "summary", "Loading summary model")) return;
              setSummaryError(null);
              try {
                const summarized = await api.summarizeRecording(recording.id);
                setTitle(summarized.title);
                onChanged(summarized);
              } catch (cause) {
                setSummaryError(cause instanceof Error ? cause.message : String(cause));
              } finally {
                onJobEnd(recording.id, "summary");
              }
            }}>{summarizing ? "Summarizing…" : recording.summary ? "Summarize again" : "Summarize"}</button>
          ) : <button className="button secondary compact-button" onClick={onChooseSummaryModel}>Choose summary model</button>)}
        </div>
        {summaryError && <p className="player-error" role="alert">{summaryError}</p>}
        {recording.summary ? (
          <div className="summary-body">
            <p className="summary-overview">{recording.summary.overview}</p>
            {recording.summary.suggestedTitle && recording.summary.suggestedTitle !== recording.title && (
              <div className="summary-title-suggestion">
                <div><strong>Suggested title</strong><span>{recording.summary.suggestedTitle}</span></div>
                <button className="button secondary compact-button" disabled={busy} onClick={() => useSuggestedTitle(recording.summary!.suggestedTitle)}>Use this title</button>
              </div>
            )}
            <SummaryList heading="Key points" items={recording.summary.keyPoints} />
            <SummaryList heading="Decisions" items={recording.summary.decisions} />
            <SummaryList heading="Action items" items={recording.summary.actionItems} />
          </div>
        ) : <p className="section-empty">{!summaryModelPath
          ? "Install a local summary model to have meetings summarized on this computer."
          : recording.transcript
            ? "Summarize this meeting on-device, and get a title that says what it was about."
            : "Transcribe this recording first — the summary is written from the transcript."}</p>}
      </section>

      <section className="detail-section transcript-section">
        <div className="section-heading">
          <div><h2>Transcript</h2>{recording.transcript?.language && <span>{recording.transcript.language}</span>}</div>
          {!deleted && (whisperModelPath ? (
            <button className="button secondary compact-button" disabled={processing || recording.sizeBytes === 0} onClick={async () => {
              if (!onJobStart(recording, "transcription", "Preparing audio")) return;
              setTranscriptionError(null);
              try {
                onChanged(await api.transcribeRecording(recording.id));
              } catch (cause) {
                setTranscriptionError(cause instanceof Error ? cause.message : String(cause));
              } finally {
                onJobEnd(recording.id, "transcription");
              }
            }}>{transcribing ? "Transcribing…" : recording.transcript ? "Transcribe again" : "Transcribe"}</button>
          ) : <button className="button secondary compact-button" onClick={onChooseWhisperModel}>Choose Whisper model</button>)}
        </div>
        {transcriptionError && <p className="player-error" role="alert">{transcriptionError}</p>}
        {recording.transcript ? (
          <div className="transcript-body">
            {recording.transcript.segments.map((segment, index) => (
              <p key={`${segment.startMs}-${index}`}><time>{formatDuration(segment.startMs)}</time><span>{segment.text}</span></p>
            ))}
          </div>
        ) : <p className="section-empty">{whisperModelPath ? "Create a private, on-device transcript with Whisper." : "Choose a local open-source Whisper model to enable transcription."}</p>}
      </section>

      <section className="detail-section">
        <h2>Highlights</h2>
        {recording.highlights.length ? recording.highlights.map((highlight, index) => (
          <button className="highlight-row" key={highlight.id}><MarkerIcon /><span>Highlight {index + 1}</span><time>{formatDuration(highlight.offsetMs)}</time></button>
        )) : <p className="section-empty">No highlights were added during this recording.</p>}
      </section>

      <section className="detail-section recording-info">
        <h2>Recording information</h2>
        <dl><div><dt>Mode</dt><dd>{recording.mode === "online" ? "Microphone + computer" : "Microphone"}</dd></div><div><dt>Format</dt><dd>{recording.codec || "AAC-LC"}</dd></div><div><dt>Status</dt><dd>{recording.status === "recovered" ? "Recovered after interruption" : "Stored securely"}</dd></div></dl>
      </section>

      {!deleted && <button className="danger-link" disabled={busy} onClick={async () => { if (window.confirm("Move this recording to Recently Deleted?")) { await api.deleteRecording(recording.id); onRemoved(); } }}><TrashIcon /> Move to Recently Deleted</button>}
    </div>
  );
}

function SummaryList({ heading, items }: { heading: string; items: string[] }) {
  if (!items.length) return null;
  return (
    <div className="summary-list">
      <h3>{heading}</h3>
      <ul>{items.map((item, index) => <li key={`${heading}-${index}`}>{item}</li>)}</ul>
    </div>
  );
}

function EncryptedAudioPlayer({ recording }: { recording: Recording }) {
  const audio = useRef<HTMLAudioElement | null>(null);
  const objectUrl = useRef<string | null>(null);
  const [playing, setPlaying] = useState(false);
  const [loading, setLoading] = useState(false);
  const [position, setPosition] = useState(0);
  const [playbackError, setPlaybackError] = useState<string | null>(null);
  const durationSeconds = Math.max(0.001, recording.playableDurationMs / 1000);

  useEffect(() => () => {
    audio.current?.pause();
    if (objectUrl.current) URL.revokeObjectURL(objectUrl.current);
  }, []);

  const toggle = async () => {
    setPlaybackError(null);
    try {
      if (!audio.current) {
        setLoading(true);
        const bytes = await api.getRecordingAudio(recording.id);
        if (!bytes?.length) throw new Error("The recording contains no playable audio.");
        const audioBuffer = bytes.slice().buffer as ArrayBuffer;
        objectUrl.current = URL.createObjectURL(new Blob([audioBuffer], { type: "audio/mp4" }));
        const element = new Audio(objectUrl.current);
        element.preload = "auto";
        element.addEventListener("timeupdate", () => setPosition(element.currentTime));
        element.addEventListener("ended", () => setPlaying(false));
        element.addEventListener("pause", () => setPlaying(false));
        element.addEventListener("play", () => setPlaying(true));
        element.addEventListener("error", () => setPlaybackError("macOS could not decode this recording."));
        audio.current = element;
      }
      if (audio.current.paused) await audio.current.play(); else audio.current.pause();
    } catch (cause) {
      setPlaybackError(cause instanceof Error ? cause.message : "Playback could not start.");
    } finally {
      setLoading(false);
    }
  };

  const seek = (event: React.ChangeEvent<HTMLInputElement>) => {
    const next = Number(event.target.value);
    setPosition(next);
    if (audio.current) audio.current.currentTime = next;
  };

  return (
    <div className="player-stack">
      <div className="player">
        <button className="player-button" disabled={loading || recording.sizeBytes === 0} onClick={toggle} aria-label={playing ? "Pause" : "Play"}>{playing ? <PauseIcon /> : <PlayIcon />}</button>
        <span className="player-time">{formatDuration(position * 1000)}</span>
        <input className="player-range" type="range" min="0" max={durationSeconds} step="0.1" value={Math.min(position, durationSeconds)} onChange={seek} aria-label="Playback position" />
        <span className="player-time">{formatDuration(recording.playableDurationMs)}</span>
      </div>
      {playbackError && <p className="player-error" role="alert">{playbackError}</p>}
    </div>
  );
}

function SettingsPage({ app, updater, models }: { app: AppController; updater: AppUpdater; models: ModelManager }) {
  const snapshot = app.snapshot!;
  const [selectedWhisperModel, setSelectedWhisperModel] = useState("base");
  const [selectedSummaryModel, setSelectedSummaryModel] = useState("qwen2.5-1.5b");
  const [summaryPrompt, setSummaryPrompt] = useState(snapshot.settings.summaryPrompt ?? DEFAULT_SUMMARY_PROMPT);
  const [savingSummaryPrompt, setSavingSummaryPrompt] = useState(false);
  const recordingActive = ["starting", "recording", "paused", "finalizing"].includes(snapshot.session.phase);
  const update = updater.state;
  const updateDescription = update.phase === "available"
    ? `Version ${update.availableVersion} is ready to download.`
    : update.phase === "downloading"
      ? update.progress === null ? "Downloading update…" : `Downloading update… ${update.progress}%`
      : update.phase === "up_to_date"
        ? "You have the latest version."
        : update.phase === "error"
          ? update.error ?? "The update check failed."
          : "Eavesdrop checks for signed updates when it starts.";
  useEffect(() => {
    const active = models.whisperModels.find((model) => modelPathContainsId(snapshot.settings.whisperModelPath, model.id));
    if (active) setSelectedWhisperModel(active.id);
  }, [models.whisperModels, snapshot.settings.whisperModelPath]);
  useEffect(() => {
    const active = models.summaryModels.find((model) => modelPathContainsId(snapshot.settings.summaryModelPath, model.id));
    if (active) setSelectedSummaryModel(active.id);
  }, [models.summaryModels, snapshot.settings.summaryModelPath]);
  useEffect(() => {
    setSummaryPrompt(snapshot.settings.summaryPrompt ?? DEFAULT_SUMMARY_PROMPT);
  }, [snapshot.settings.summaryPrompt]);
  const selectedModel = models.whisperModels.find((model) => model.id === selectedWhisperModel);
  const selectedSummary = models.summaryModels.find((model) => model.id === selectedSummaryModel);
  const percentageOf = (progress: ModelDownloadStatus | null) => progress?.totalBytes
    ? Math.min(100, Math.round(progress.downloadedBytes / progress.totalBytes * 100))
    : 0;
  const installingModel = models.download?.kind === "whisper";
  const installingSummaryModel = models.download?.kind === "summary";
  const modelPercentage = percentageOf(installingModel ? models.download : null);
  const summaryModelPercentage = percentageOf(installingSummaryModel ? models.download : null);
  const modelBusy = models.download !== null;
  const whisperInUse = selectedModel ? modelPathContainsId(snapshot.settings.whisperModelPath, selectedModel.id) : false;
  const summaryInUse = selectedSummary ? modelPathContainsId(snapshot.settings.summaryModelPath, selectedSummary.id) : false;
  const normalizedSummaryPrompt = summaryPrompt.trim();
  const storedSummaryPrompt = snapshot.settings.summaryPrompt ?? DEFAULT_SUMMARY_PROMPT;
  const summaryPromptChanged = normalizedSummaryPrompt !== storedSummaryPrompt;
  const saveSummaryPrompt = async (prompt: string) => {
    const normalized = prompt.trim();
    if (!normalized) return;
    setSavingSummaryPrompt(true);
    await app.updateSettings({ summaryPrompt: normalized });
    setSavingSummaryPrompt(false);
  };
  return (
    <div className="settings-page">
      <header className="content-header"><div><h1>Settings</h1><p>Recording sources and app behavior.</p></div></header>
      <section className="settings-section">
        <h2>Appearance</h2>
        <label className="field-label">Theme<select value={snapshot.settings.theme} onChange={(event) => app.updateSettings({ theme: event.target.value as AppSnapshot["settings"]["theme"] })}><option value="light">Light</option><option value="dark">Dark</option><option value="system">System</option></select></label>
      </section>
      <section className="settings-section">
        <h2>Recording</h2>
        <label className="field-label">Microphone<select value={snapshot.settings.microphoneId ?? ""} onChange={(event) => app.updateSettings({ microphoneId: event.target.value || null })}><option value="">System default</option>{snapshot.devices.map((device) => <option key={device.id} value={device.id}>{device.name}</option>)}</select></label>
        <SettingToggle label="Meeting detection" description="Prompt for Zoom, Teams, and Google Meet calls." checked={snapshot.settings.meetingDetectionEnabled} onChange={(meetingDetectionEnabled) => app.updateSettings({ meetingDetectionEnabled })} />
      </section>
      <section className="settings-section">
        <h2>Local transcription</h2>
        <div className="model-setting">
          <div><strong>Open-source Whisper</strong><p>{snapshot.settings.whisperModelPath ? `Using ${modelFileName(snapshot.settings.whisperModelPath)}` : "Install a model once, then transcribe without sending audio anywhere."}</p></div>
          <div className="model-install-controls">
            <select value={selectedWhisperModel} onChange={(event) => setSelectedWhisperModel(event.target.value)} aria-label="Whisper model">
              {models.whisperModels.map((model) => <option key={model.id} value={model.id}>{model.name} · {formatSize(model.sizeBytes)}{model.installed ? " · Installed" : ""}</option>)}
            </select>
            <button className="button primary" disabled={!selectedModel || modelBusy || whisperInUse} onClick={() => selectedModel?.installed ? models.useWhisper(selectedWhisperModel) : models.installWhisper(selectedWhisperModel)}>{installingModel ? `${modelPercentage}%` : whisperInUse ? "In use" : selectedModel?.installed ? "Use model" : "Install"}</button>
            {selectedModel?.installed && <button className="button model-delete" disabled={modelBusy} onClick={() => {
              if (window.confirm(`Delete ${selectedModel.name} from this device?`)) void models.removeWhisper(selectedWhisperModel);
            }}>Delete</button>}
            <button className="button secondary" disabled={modelBusy} onClick={async () => {
              const path = await api.selectWhisperModel();
              if (path) await app.updateSettings({ whisperModelPath: path });
            }}>Choose existing…</button>
          </div>
          {selectedModel && <p className="model-description">{selectedModel.description}</p>}
          {installingModel && <progress className="update-progress" max={100} value={modelPercentage} aria-label="Whisper model download progress" />}
        </div>
      </section>
      <section className="settings-section">
        <h2>Local summaries</h2>
        <div className="model-setting">
          <div><strong>Open-weight language model</strong><p>{snapshot.settings.summaryModelPath ? `Using ${modelFileName(snapshot.settings.summaryModelPath)}` : "Summaries and suggested titles are written on this computer, from the transcript."}</p></div>
          <div className="model-install-controls">
            <select value={selectedSummaryModel} onChange={(event) => setSelectedSummaryModel(event.target.value)} aria-label="Summary model">
              {models.summaryModels.map((model) => <option key={model.id} value={model.id}>{model.name} · {formatSize(model.sizeBytes)}{model.installed ? " · Installed" : ""}</option>)}
            </select>
            <button className="button primary" disabled={!selectedSummary || modelBusy || summaryInUse} onClick={() => selectedSummary?.installed ? models.useSummary(selectedSummaryModel) : models.installSummary(selectedSummaryModel)}>{installingSummaryModel ? `${summaryModelPercentage}%` : summaryInUse ? "In use" : selectedSummary?.installed ? "Use model" : "Install"}</button>
            {selectedSummary?.installed && <button className="button model-delete" disabled={modelBusy} onClick={() => {
              if (window.confirm(`Delete ${selectedSummary.name} from this device?`)) void models.removeSummary(selectedSummaryModel);
            }}>Delete</button>}
            <button className="button secondary" disabled={modelBusy} onClick={async () => {
              const path = await api.selectSummaryModel();
              if (path) await app.updateSettings({ summaryModelPath: path });
            }}>Choose existing…</button>
          </div>
          {selectedSummary && <p className="model-description">{selectedSummary.description}</p>}
          {installingSummaryModel && <progress className="update-progress" max={100} value={summaryModelPercentage} aria-label="Summary model download progress" />}
        </div>
        <div className="summary-prompt-setting">
          <label htmlFor="summary-prompt">Summary prompt</label>
          <p>Choose what the summary should emphasize and how it should be written. The title, overview, key points, decisions, and action items sections are kept so Eavesdrop can display the result.</p>
          <textarea
            id="summary-prompt"
            value={summaryPrompt}
            maxLength={6000}
            rows={6}
            onChange={(event) => setSummaryPrompt(event.target.value)}
          />
          <div className="summary-prompt-actions">
            <button className="button secondary" disabled={savingSummaryPrompt || (normalizedSummaryPrompt === DEFAULT_SUMMARY_PROMPT && storedSummaryPrompt === DEFAULT_SUMMARY_PROMPT)} onClick={() => {
              setSummaryPrompt(DEFAULT_SUMMARY_PROMPT);
              void saveSummaryPrompt(DEFAULT_SUMMARY_PROMPT);
            }}>Use default</button>
            <button className="button primary" disabled={savingSummaryPrompt || !normalizedSummaryPrompt || !summaryPromptChanged} onClick={() => void saveSummaryPrompt(summaryPrompt)}>{savingSummaryPrompt ? "Saving…" : "Save prompt"}</button>
          </div>
        </div>
      </section>
      <section className="settings-section">
        <h2>General</h2>
        <SettingToggle label="Launch at login" description="Keep the recorder available in the menu bar or system tray." checked={snapshot.settings.launchAtLogin} onChange={(launchAtLogin) => app.updateSettings({ launchAtLogin })} />
      </section>
      <section className="settings-section">
        <h2>Updates</h2>
        <div className="update-setting">
          <div><strong>Eavesdrop {update.currentVersion || ""}</strong><p>{recordingActive && update.phase === "available" ? "Stop the active recording before updating." : update.error && update.phase === "available" ? update.error : updateDescription}</p></div>
          {update.phase === "available" || update.phase === "downloading" ? (
            <button className="button primary" disabled={recordingActive || update.phase === "downloading"} onClick={updater.installUpdate}>
              {update.phase === "downloading" ? `${update.progress ?? 0}%` : "Update & restart"}
            </button>
          ) : (
            <button className="button secondary" disabled={update.phase === "checking"} onClick={updater.checkForUpdates}>{update.phase === "checking" ? "Checking…" : "Check for updates"}</button>
          )}
        </div>
        {update.phase === "downloading" && <progress className="update-progress" max={100} value={update.progress ?? undefined} aria-label="Update download progress" />}
      </section>
      <section className="settings-section">
        <h2>Permissions</h2>
        <PermissionRow label="Microphone" state={snapshot.permissions.microphone} />
        <PermissionRow label="Computer audio" state={snapshot.permissions.systemAudio} />
        <button className="button secondary" onClick={app.requestPermissions}>Check permissions</button>
      </section>
      <section className="settings-section">
        <h2>Diagnostics</h2>
        <p className="settings-description">Local event logs exclude audio, keys, meeting titles, and window contents. Logs expire after seven days.</p>
        <button className="button secondary" onClick={() => api.exportDiagnostics()}>Export diagnostics</button>
      </section>
    </div>
  );
}

function modelFileName(path: string) {
  return path.split(/[\\/]/).pop() || path;
}

function modelPathContainsId(path: string | null, modelId: string) {
  return path ? modelFileName(path).toLocaleLowerCase().includes(modelId.toLocaleLowerCase()) : false;
}

function UpdateBanner({ app, updater }: { app: AppController; updater: AppUpdater }) {
  const update = updater.state;
  if (update.phase !== "available" && update.phase !== "downloading") return null;
  const recordingActive = ["starting", "recording", "paused", "finalizing"].includes(app.snapshot!.session.phase);
  return (
    <div className="update-banner" role="status">
      <div><strong>Eavesdrop {update.availableVersion} is available</strong><span>{recordingActive ? "Stop recording before updating." : update.phase === "downloading" ? `Downloading… ${update.progress ?? 0}%` : update.error ?? "The app will restart after installation."}</span></div>
      <button className="button primary" disabled={recordingActive || update.phase === "downloading"} onClick={updater.installUpdate}>{update.phase === "downloading" ? `${update.progress ?? 0}%` : "Update now"}</button>
    </div>
  );
}

function Onboarding({ app }: { app: AppController }) {
  const snapshot = app.snapshot!;
  const [step, setStep] = useState(0);
  const [launchAtLogin, setLaunchAtLogin] = useState(true);
  const granted = snapshot.permissions.microphone === "granted" && snapshot.permissions.systemAudio === "granted";
  const screenDenied = snapshot.permissions.systemAudio === "denied";
  return (
    <main className="onboarding">
      <div className="onboarding-bar" onMouseDown={beginWindowDrag}><div className="wordmark"><BrandMark /> Eavesdrop</div><span>Setup {step + 1} of 3</span></div>
      {step === 0 && <section className="onboarding-content"><div className="onboarding-icon"><RecordingsIcon /></div><h1>Record the room and the call</h1><p>Eavesdrop captures in-person conversations through your microphone and online meetings directly from your computer—without joining as a bot.</p><ul className="plain-checks"><li>Recordings stay encrypted on this computer</li><li>Nothing is uploaded automatically</li><li>A red indicator is always visible while recording</li></ul><div className="onboarding-actions"><button className="button primary" onClick={() => setStep(1)}>Continue</button></div></section>}
      {step === 1 && <section className="onboarding-content"><div className="onboarding-icon"><MicIcon /></div><h1>Allow recording access</h1><p>Microphone access records people near you. Computer audio access records voices from Zoom, Teams, and Google Meet, including when headphones are connected.</p><div className="permission-list"><PermissionRow label="Microphone" state={snapshot.permissions.microphone} /><PermissionRow label="Computer audio" state={snapshot.permissions.systemAudio} /></div><div className="onboarding-actions"><button className="button secondary" onClick={() => setStep(0)}>Back</button><button className="button primary" onClick={async () => { if (screenDenied) await api.openScreenRecordingSettings(); else await app.requestPermissions(); }}>{screenDenied ? "Open Settings" : "Allow access"}</button><button className="button secondary" disabled={!granted} onClick={() => setStep(2)}>Continue</button></div></section>}
      {step === 2 && <section className="onboarding-content"><div className="onboarding-icon"><SettingsIcon /></div><h1>Keep it within reach</h1><p>Eavesdrop works best when it starts quietly with your computer and waits in the menu bar or system tray.</p><SettingToggle label="Launch Eavesdrop at login" description="You can change this later in Settings." checked={launchAtLogin} onChange={setLaunchAtLogin} /><div className="consent-box"><strong>Recording responsibly</strong><p>Make sure everyone knows and agrees before you start. Recording laws differ by location.</p></div><div className="onboarding-actions"><button className="button secondary" onClick={() => setStep(1)}>Back</button><button className="button primary" onClick={() => app.completeOnboarding(launchAtLogin)}>Finish setup</button></div></section>}
      {app.error && <InlineError message={app.error} onClose={app.clearError} />}
    </main>
  );
}

function SettingToggle({ label, description, checked, onChange }: { label: string; description: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return <label className="setting-toggle"><span><strong>{label}</strong><em>{description}</em></span><input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} /><i aria-hidden="true" /></label>;
}

function PermissionRow({ label, state }: { label: string; state: string }) {
  const ready = state === "granted";
  return <div className="permission-row"><span>{label}</span><strong className={ready ? "permission-granted" : "permission-needed"}>{ready ? "Allowed" : state === "unavailable" ? "Unavailable" : "Needs access"}</strong></div>;
}

function BrandMark({ active = false }: { active?: boolean }) {
  return <span className={active ? "brand-mark active" : "brand-mark"} aria-hidden="true"><i /><i /><i /><i /><i /></span>;
}

function InlineError({ message, onClose }: { message: string; onClose: () => void }) {
  const screenPermission = /screen recording|computer audio access|TCC/i.test(message);
  return <div className="inline-error" role="alert"><span>{message}</span><div className="inline-error-actions">{screenPermission && <button className="inline-error-action" onClick={() => api.openScreenRecordingSettings()}>Open Settings</button>}<button className="inline-error-dismiss" onClick={onClose} aria-label="Dismiss">×</button></div></div>;
}
