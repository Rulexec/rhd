export function formatDateTime(isoString: string): string {
  const d = new Date(isoString);
  const pad = (n: number): string => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function formatDuration(durationMs: number): string {
  const totalSeconds = Math.floor(durationMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
}

export function formatCost(cost: number | null | undefined): string {
  if (cost === null || cost === undefined) return '—';
  return `$${cost.toFixed(4)}`;
}

export function elapsedSeconds(startedAtIso: string): number {
  const started = new Date(startedAtIso).getTime();
  const now = Date.now();
  return Math.floor((now - started) / 1000);
}
