import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement>;

const base = (props: IconProps) => ({
  width: 18,
  height: 18,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.8,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
  ...props,
});

export function RecordingsIcon(props: IconProps) {
  return <svg {...base(props)}><path d="M5 8v8M9 5v14M13 8v8M17 3v18M21 9v6" /></svg>;
}

export function TrashIcon(props: IconProps) {
  return <svg {...base(props)}><path d="M4 7h16M9 7V4h6v3M7 7l1 14h8l1-14M10 11v6M14 11v6" /></svg>;
}

export function SettingsIcon(props: IconProps) {
  return <svg {...base(props)}><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 .6 1.7 1.7 0 0 0-.4 1.1V21H9.6v-.1A1.7 1.7 0 0 0 8.5 19.4a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.1 15a1.7 1.7 0 0 0-.6-1 1.7 1.7 0 0 0-1.1-.4H2.3V9.6h.1A1.7 1.7 0 0 0 4.1 8.5a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 8.5 4.1a1.7 1.7 0 0 0 1-.6 1.7 1.7 0 0 0 .4-1.1V2.3h4v.1A1.7 1.7 0 0 0 15 4.1a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.4 8.5a1.7 1.7 0 0 0 .6 1 1.7 1.7 0 0 0 1.1.4h.1v4h-.1A1.7 1.7 0 0 0 19.4 15Z" /></svg>;
}

export function IntegrationsIcon(props: IconProps) {
  return <svg {...base(props)}><path d="M8 12h8M7 8H5a3 3 0 0 0 0 6h2M17 8h2a3 3 0 0 1 0 6h-2" /><path d="M9 5v3M15 5v3M9 16v3M15 16v3" /></svg>;
}

export function MicIcon(props: IconProps) {
  return <svg {...base(props)}><rect x="9" y="3" width="6" height="11" rx="3" /><path d="M5.5 11.5a6.5 6.5 0 0 0 13 0M12 18v3M9 21h6" /></svg>;
}

export function ComputerIcon(props: IconProps) {
  return <svg {...base(props)}><rect x="3" y="4" width="18" height="13" rx="2" /><path d="M8 21h8M12 17v4" /></svg>;
}

export function PlayIcon(props: IconProps) {
  return <svg {...base(props)}><path d="m8 5 11 7-11 7Z" fill="currentColor" stroke="none" /></svg>;
}

export function PauseIcon(props: IconProps) {
  return <svg {...base(props)}><path d="M8 5v14M16 5v14" strokeWidth="2.5" /></svg>;
}

export function StopIcon(props: IconProps) {
  return <svg {...base(props)}><rect x="6" y="6" width="12" height="12" rx="1.5" fill="currentColor" stroke="none" /></svg>;
}

export function MarkerIcon(props: IconProps) {
  return <svg {...base(props)}><path d="M6 3v18M7 4h10l-2.5 4L17 12H7" /></svg>;
}

export function SearchIcon(props: IconProps) {
  return <svg {...base(props)}><circle cx="11" cy="11" r="7" /><path d="m16 16 5 5" /></svg>;
}

export function ExportIcon(props: IconProps) {
  return <svg {...base(props)}><path d="M12 3v12M7 8l5-5 5 5M5 14v6h14v-6" /></svg>;
}

export function MoreIcon(props: IconProps) {
  return <svg {...base(props)}><circle cx="5" cy="12" r="1" fill="currentColor" /><circle cx="12" cy="12" r="1" fill="currentColor" /><circle cx="19" cy="12" r="1" fill="currentColor" /></svg>;
}

export function ChevronLeftIcon(props: IconProps) {
  return <svg {...base(props)}><path d="m15 18-6-6 6-6" /></svg>;
}
