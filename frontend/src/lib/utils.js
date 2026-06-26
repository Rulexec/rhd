export function formatDateTime(isoString) {
  const date = new Date(isoString);
  return date.toLocaleString();
}

export function formatDuration(durationMs) {
  const totalSeconds = Math.floor(durationMs / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
}

export function formatCost(cost) {
  if (cost === null || cost === undefined) return '—';
  return `$${cost.toFixed(4)}`;
}

export function elapsedSeconds(startedAtIso) {
  const started = new Date(startedAtIso).getTime();
  const now = Date.now();
  return Math.floor((now - started) / 1000);
}
