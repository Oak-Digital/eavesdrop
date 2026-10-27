import { useId, useRef, type ReactNode } from "react";

const tabs = [
  { id: "general", label: "General" },
  { id: "recording", label: "Recording" },
  { id: "transcription", label: "Transcription" },
  { id: "summaries", label: "Summaries" },
  { id: "about", label: "About" },
] as const;

export type SettingsTab = (typeof tabs)[number]["id"];

export function SettingsTabs({ value, onValueChange, panels }: {
  value: SettingsTab;
  onValueChange: (value: SettingsTab) => void;
  panels: Record<SettingsTab, ReactNode>;
}) {
  const id = useId();
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  return <>
    <div className="settings-tabs" role="tablist" aria-label="Settings sections">
      {tabs.map((tab, index) => (
        <button key={tab.id} ref={(element) => { buttons.current[index] = element; }}
          type="button" role="tab" id={`${id}-tab-${tab.id}`}
          aria-controls={`${id}-panel-${tab.id}`} aria-selected={value === tab.id}
          tabIndex={value === tab.id ? 0 : -1}
          onClick={() => onValueChange(tab.id)}
          onKeyDown={(event) => {
            let next: number;
            if (event.key === "ArrowRight") next = (index + 1) % tabs.length;
            else if (event.key === "ArrowLeft") next = (index + tabs.length - 1) % tabs.length;
            else if (event.key === "Home") next = 0;
            else if (event.key === "End") next = tabs.length - 1;
            else return;
            event.preventDefault();
            onValueChange(tabs[next].id);
            buttons.current[next]?.focus();
          }}>{tab.label}</button>
      ))}
    </div>
    {tabs.map((tab) => (
      <div key={tab.id} className="settings-panel" role="tabpanel"
        id={`${id}-panel-${tab.id}`} aria-labelledby={`${id}-tab-${tab.id}`}
        tabIndex={0} hidden={value !== tab.id}>
        {panels[tab.id]}
      </div>
    ))}
  </>;
}
