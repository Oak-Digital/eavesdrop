export function formatDuration(milliseconds: number): string {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return [hours, minutes, seconds].map((part) => String(part).padStart(2, "0")).join(":");
  return [minutes, seconds].map((part) => String(part).padStart(2, "0")).join(":");
}

export function formatDate(value: string): string {
  const date = new Date(value);
  const today = new Date();
  const sameDay = date.toDateString() === today.toDateString();
  if (sameDay) return `Today, ${date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
  return date.toLocaleString([], { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${Math.round(bytes / 1024)} KB`;
  // Summary models run to several gigabytes, where "4466.1 MB" stops being a
  // size a reader can weigh against their free disk space.
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

// Converts a 0..1 transcription progress ratio into a whole percentage,
// tolerating out-of-range values from the backend.
export function transcriptionPercentage(progress: number): number {
  if (Number.isNaN(progress)) return 0;
  return Math.round(Math.min(Math.max(progress, 0), 1) * 100);
}
