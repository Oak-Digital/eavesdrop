import { useEffect, useId, useMemo, useState } from "react";
import type { OakOsProject } from "./types";

const normalize = (text: string) => text.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLocaleLowerCase();
const compare = (left: string, right: string) => left.localeCompare(right, undefined, { sensitivity: "base", numeric: true });

export function OakOsProjectPicker({ id, projects, value, onValueChange, disabled, autoFocus }: {
  id: string;
  projects: OakOsProject[];
  value: string;
  onValueChange: (value: string) => void;
  disabled?: boolean;
  autoFocus?: boolean;
}) {
  const listId = useId();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(-1);
  const selected = projects.find((project) => project.id === value);
  const expanded = open && !disabled;
  const groups = useMemo(() => {
    const terms = normalize(query).trim().split(/\s+/).filter(Boolean);
    const grouped = new Map<string, { key: string; name: string; projects: OakOsProject[]; unassigned: boolean }>();
    for (const project of projects) {
      const clientName = project.client?.name.trim() || "No client";
      const searchable = normalize(`${project.name} ${clientName} ${project.identifier ?? ""}`);
      if (!terms.every((term) => searchable.includes(term))) continue;
      const key = project.client ? `client:${project.client.id}` : "unassigned";
      const group = grouped.get(key) ?? { key, name: clientName, projects: [], unassigned: !project.client };
      group.projects.push(project);
      grouped.set(key, group);
    }
    return [...grouped.values()]
      .sort((a, b) => Number(a.unassigned) - Number(b.unassigned) || compare(a.name, b.name) || compare(a.key, b.key))
      .map((group) => ({ ...group, projects: group.projects.sort((a, b) => compare(a.name, b.name) || compare(a.identifier ?? "", b.identifier ?? "") || compare(a.id, b.id)) }));
  }, [projects, query]);
  const options = groups.flatMap((group) => group.projects);
  const active = options[activeIndex];
  const optionId = (project: OakOsProject) => `${listId}-${encodeURIComponent(project.id)}`;
  useEffect(() => {
    if (expanded && active) document.getElementById(`${listId}-${encodeURIComponent(active.id)}`)?.scrollIntoView?.({ block: "nearest" });
  }, [expanded, active?.id, listId]);

  const close = () => { setOpen(false); setQuery(""); setActiveIndex(-1); };
  const show = () => { if (!open) { setQuery(""); setActiveIndex(-1); } setOpen(true); };
  const choose = (project: OakOsProject) => { onValueChange(project.id); close(); };
  const selectedLabel = selected ? `${selected.name} · ${selected.client?.name || "No client"}` : "";

  return <div className="project-picker" onBlur={(event) => {
    if (!event.currentTarget.contains(event.relatedTarget as Node | null)) close();
  }}>
    <input id={id} role="combobox" aria-label="Project" aria-autocomplete="list" aria-expanded={expanded}
      aria-controls={expanded ? listId : undefined} aria-activedescendant={expanded && active ? optionId(active) : undefined}
      autoFocus={autoFocus} autoComplete="off" spellCheck={false} disabled={disabled}
      placeholder="Search projects or clients…" value={expanded ? query : selectedLabel}
      onFocus={show} onClick={show}
      onChange={(event) => { setQuery(event.target.value); setActiveIndex(-1); setOpen(true); if (value) onValueChange(""); }}
      onKeyDown={(event) => {
        if (event.nativeEvent.isComposing) return;
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();
          if (!expanded) { show(); return; }
          const direction = event.key === "ArrowDown" ? 1 : -1;
          setActiveIndex((current) => options.length ? current < 0 ? (direction === 1 ? 0 : options.length - 1) : (current + direction + options.length) % options.length : -1);
        } else if (event.key === "Enter" && expanded) {
          event.preventDefault(); event.stopPropagation();
          if (active) choose(active);
        } else if (event.key === "Escape" && expanded) {
          event.preventDefault(); event.stopPropagation(); close();
        }
      }} />
    {expanded && <div className="project-picker-menu">
      <div id={listId} role="listbox" aria-label="Projects by client" className="project-picker-options">
        {groups.map((group, index) => <div key={group.key} role="group" aria-labelledby={`${listId}-group-${index}`}>
          <div className="project-picker-client" id={`${listId}-group-${index}`}>{group.name}</div>
          {group.projects.map((project) => <div key={project.id} id={optionId(project)} role="option"
            aria-selected={value === project.id} className="project-picker-option" data-active={active?.id === project.id}
            onMouseDown={(event) => event.preventDefault()} onClick={() => choose(project)}
            onMouseMove={() => setActiveIndex(options.findIndex((option) => option.id === project.id))}>
            <span>{project.name}</span>
            {project.identifier && <span className="project-picker-code">{project.identifier}</span>}
            {value === project.id && <span aria-hidden="true">✓</span>}
          </div>)}
        </div>)}
      </div>
      {!options.length && <p className="project-picker-empty" role="status">{projects.length ? "No matching projects." : "No projects available."}</p>}
    </div>}
  </div>;
}
