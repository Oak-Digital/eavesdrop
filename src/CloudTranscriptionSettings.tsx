import { useEffect, useState } from "react";
import * as api from "./api";
import { Select } from "./Select";
import { CLOUD_TRANSCRIPTION_MODEL, TRANSCRIPTION_SOURCE_OPTIONS } from "./cloudModels";
import type { AppSettings, CloudKeyStatus, TranscriptionSource } from "./types";

export function CloudTranscriptionSettings({ settings, keys, keyError, onSave, onKeysChanged }: {
  settings: AppSettings;
  keys: CloudKeyStatus[];
  keyError: string | null;
  onSave: (patch: Partial<AppSettings>) => Promise<unknown>;
  onKeysChanged: () => Promise<void>;
}) {
  const [expanded, setExpanded] = useState(settings.transcriptionSource === "cloud");
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { setExpanded(settings.transcriptionSource === "cloud"); }, [settings.transcriptionSource]);
  const stored = keys.some((key) => key.provider === "openai" && key.keyStored);
  const run = async (action: () => Promise<unknown>) => {
    setBusy(true); setError(null);
    try { await action(); }
    catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); }
    finally { setBusy(false); }
  };

  return <section className="settings-section">
    <h2>Transcription processing</h2>
    <div className="summary-default-setting">
      <label htmlFor="default-transcription-source">Default processing</label>
      <Select id="default-transcription-source" label="Default transcription processing" value={settings.transcriptionSource}
        options={TRANSCRIPTION_SOURCE_OPTIONS} disabled={busy} onValueChange={(value) => void run(() => onSave({ transcriptionSource: value as TranscriptionSource }))} />
    </div>
    <p className="settings-description">Choose a default, or change processing for an individual recording before transcribing.</p>
    <details className="settings-model-details" open={expanded}>
      <summary onClick={(event) => { event.preventDefault(); setExpanded(!expanded); }}>Cloud transcription<span>OpenAI</span></summary>
      <div className="model-setting summary-model-group" role="group" aria-label="Cloud transcription">
        <div><strong>OpenAI · {CLOUD_TRANSCRIPTION_MODEL}</strong><p>Audio is sent to OpenAI when you transcribe with Cloud, including after saving if Auto transcribe is enabled. Provider charges may apply. Long recordings are split automatically; cloud transcripts do not include timestamps.</p></div>
        <div className="cloud-field">
          <label htmlFor="transcription-api-key">OpenAI transcription API key</label>
          <div className="cloud-key-controls">
            <input id="transcription-api-key" type="password" value={apiKey} disabled={busy} autoComplete="new-password" spellCheck={false} maxLength={4096}
              placeholder={stored ? "Enter a replacement key" : "Paste your OpenAI API key"} onChange={(event) => setApiKey(event.target.value)} />
            <button className="button secondary" disabled={busy || !apiKey.trim()} onClick={() => void run(async () => {
              await api.saveCloudApiKey("openai", apiKey); setApiKey(""); await onKeysChanged();
            })}>{stored ? "Replace key" : "Save key"}</button>
            {stored && <button className="button secondary" disabled={busy} onClick={() => void run(async () => {
              await api.removeCloudApiKey("openai"); setApiKey(""); await onKeysChanged();
            })}>Remove key</button>}
          </div>
          <p>{stored ? "OpenAI key saved. " : "No OpenAI key saved. "}This key is shared with OpenAI summaries and stored in your system credential store. Removing it disables both cloud features until a key is saved again.</p>
        </div>
        {keyError && <p className="player-error" role="alert">{keyError}</p>}
        {keyError && <button className="button secondary" disabled={busy} onClick={() => void run(onKeysChanged)}>Retry key status</button>}
        <button className="button primary" disabled={busy || !stored || settings.transcriptionSource === "cloud"} onClick={() => void run(() => onSave({ transcriptionSource: "cloud" }))}>
          {settings.transcriptionSource === "cloud" ? "Cloud default" : "Use cloud transcription"}
        </button>
      </div>
    </details>
    {error && <p className="player-error" role="alert">{error}</p>}
  </section>;
}
