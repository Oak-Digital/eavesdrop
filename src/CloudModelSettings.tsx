import { useEffect, useState } from "react";
import * as api from "./api";
import { Select } from "./Select";
import { CLOUD_MODELS, CLOUD_PROVIDERS, providerName } from "./cloudModels";
import type { AppSettings, CloudKeyStatus, CloudProvider } from "./types";

export function CloudModelSettings({ settings, keys, keyError, onSave, onKeysChanged }: {
  settings: AppSettings;
  keys: CloudKeyStatus[];
  keyError: string | null;
  onSave: (patch: Partial<AppSettings>) => Promise<unknown>;
  onKeysChanged: () => Promise<void>;
}) {
  const [provider, setProvider] = useState(settings.cloudProvider);
  const [model, setModel] = useState(settings.cloudModel);
  const [custom, setCustom] = useState(!CLOUD_MODELS[settings.cloudProvider].some((item) => item.value === settings.cloudModel));
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setProvider(settings.cloudProvider);
    setModel(settings.cloudModel);
    setCustom(!CLOUD_MODELS[settings.cloudProvider].some((item) => item.value === settings.cloudModel));
    setApiKey("");
  }, [settings.cloudProvider, settings.cloudModel]);
  const stored = keys.some((key) => key.provider === provider && key.keyStored);
  const active = settings.summarySource === "cloud" && settings.cloudProvider === provider && settings.cloudModel === model.trim();
  const validModel = /^[A-Za-z0-9_.:/-]{1,160}$/.test(model.trim());
  const run = async (action: () => Promise<unknown>) => {
    setBusy(true); setError(null);
    try { await action(); }
    catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); }
    finally { setBusy(false); }
  };
  return <div className="model-setting summary-model-group" role="group" aria-label="Cloud">
    <div><strong>Cloud</strong><p>Use your API key. The transcript is sent to the selected provider when you generate a summary. Audio stays local. Provider charges may apply.</p></div>
    <div className="cloud-model-fields">
      <div className="cloud-field">
        <label htmlFor="cloud-provider">Provider</label>
        <Select id="cloud-provider" label="Cloud provider" value={provider} options={CLOUD_PROVIDERS} disabled={busy}
          onValueChange={(value) => { const next = value as CloudProvider; setProvider(next); setModel(CLOUD_MODELS[next][0].value); setCustom(false); setApiKey(""); setError(null); }} />
      </div>
      <div className="cloud-field">
        <label htmlFor="cloud-model">Model</label>
        <Select id="cloud-model" label="Cloud model" value={custom ? "custom" : model} disabled={busy}
          options={[...CLOUD_MODELS[provider], { value: "custom", label: "Custom model ID…" }]}
          onValueChange={(value) => { setCustom(value === "custom"); setModel(value === "custom" ? "" : value); setError(null); }} />
      </div>
    </div>
    {custom && <div className="cloud-field">
      <label htmlFor="cloud-model-id">Custom model ID</label>
      <input id="cloud-model-id" value={model} maxLength={160} disabled={busy} autoComplete="off" spellCheck={false}
        placeholder="Model ID from your provider" onChange={(event) => setModel(event.target.value)} />
    </div>}
    <div className="cloud-field">
      <label htmlFor="cloud-api-key">{providerName(provider)} API key</label>
      <div className="cloud-key-controls">
        <input id="cloud-api-key" type="password" value={apiKey} disabled={busy} autoComplete="new-password" spellCheck={false} maxLength={4096}
          placeholder={stored ? "Enter a replacement key" : "Paste your API key"} onChange={(event) => setApiKey(event.target.value)} />
        <button className="button secondary" disabled={busy || !apiKey.trim()} onClick={() => void run(async () => {
          await api.saveCloudApiKey(provider, apiKey); setApiKey(""); await onKeysChanged();
        })}>{stored ? "Replace key" : "Save key"}</button>
        {stored && <button className="button secondary" disabled={busy} onClick={() => void run(async () => {
          await api.removeCloudApiKey(provider); setApiKey(""); await onKeysChanged();
        })}>Remove key</button>}
      </div>
      <p>{stored ? "API key saved in the system credential store. " : "No API key saved. "}Keys are kept out of settings and exports. Model access is checked when you generate a summary.</p>
    </div>
    {(error || keyError) && <p className="player-error" role="alert">{error || keyError}</p>}
    {keyError && <button className="button secondary" disabled={busy} onClick={() => void run(onKeysChanged)}>Retry key status</button>}
    <div className="model-install-controls">
      <button className="button primary" disabled={busy || !stored || !validModel || active} onClick={() => void run(() => onSave({
        summarySource: "cloud", cloudProvider: provider, cloudModel: model.trim(),
      }))}>{active ? "Cloud default" : "Use Cloud model"}</button>
    </div>
  </div>;
}
