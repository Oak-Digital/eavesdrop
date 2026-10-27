import { useEffect, useState } from "react";
import { Select } from "./Select";
import { DEFAULT_CUSTOM_PROMPT, SUMMARY_TEMPLATE_OPTIONS, summaryTemplate } from "./summaryTemplates";
import type { AppSettings } from "./types";

export function SummaryTemplateSettings({ settings, onSave }: {
  settings: AppSettings;
  onSave: (patch: Partial<AppSettings>) => Promise<unknown>;
}) {
  const [selected, setSelected] = useState(settings.summaryTemplate);
  const [customPrompt, setCustomPrompt] = useState(settings.summaryPrompt ?? DEFAULT_CUSTOM_PROMPT);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { setSelected(settings.summaryTemplate); }, [settings.summaryTemplate]);
  useEffect(() => { setCustomPrompt(settings.summaryPrompt ?? DEFAULT_CUSTOM_PROMPT); }, [settings.summaryPrompt]);
  const preset = summaryTemplate(selected)!;
  const custom = selected === "custom";
  const changed = selected !== settings.summaryTemplate || (custom && customPrompt.trim() !== (settings.summaryPrompt ?? DEFAULT_CUSTOM_PROMPT));
  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      await onSave({ summaryTemplate: selected, ...(custom ? { summaryPrompt: customPrompt.trim() } : {}) });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  };
  return <div className="summary-prompt-setting">
    <label htmlFor="default-summary-template">Default summary template</label>
    <Select id="default-summary-template" label="Default summary template" value={selected}
      options={SUMMARY_TEMPLATE_OPTIONS} disabled={saving} onValueChange={setSelected} />
    <p>Used for new summaries. You can choose a different template on each recording. Summaries follow the language of the meeting.</p>
    {custom ? <>
      <label htmlFor="summary-prompt">Custom summary prompt</label>
      <p>Your instructions shape the overview, key points, decisions, and action items.</p>
      <textarea id="summary-prompt" value={customPrompt} disabled={saving} maxLength={6000} rows={6}
        onChange={(event) => setCustomPrompt(event.target.value)} />
    </> : <details className="template-instructions">
      <summary>View template instructions</summary>
      <pre>{preset.prompt}</pre>
    </details>}
    {error && <p role="alert">{error}</p>}
    <div className="summary-prompt-actions">
      <button className="button primary" disabled={saving || !changed || (custom && !customPrompt.trim())}
        onClick={() => void save()}>{saving ? "Saving…" : "Save template"}</button>
    </div>
  </div>;
}
