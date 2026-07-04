# Date Format Change Plan

## Goal
Change date rendering from locale-dependent `6/26/2026, 11:29:56 PM` to `2026-06-26 23:29:56` (ISO-like, local time, 24h).

## Current State
- [`formatDateTime()`](frontend/src/lib/utils.js:1) uses `new Date(isoString).toLocaleString()`
- Used in [`FinishedScenario.svelte`](frontend/src/components/FinishedScenario.svelte:17) for `scenario.finished` timestamp
- `toLocaleString()` output depends on browser locale — user sees US-style format

## Change
Replace `formatDateTime()` body in `frontend/src/lib/utils.js` with manual formatting using local date components:

```js
export function formatDateTime(isoString) {
  const d = new Date(isoString);
  const pad = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}
```

`Date` methods (`getFullYear`, `getMonth`, etc.) return local time by default — matches user's UTC+3 timezone.

## Files Changed
- `frontend/src/lib/utils.js` — `formatDateTime()` function only

## No Impact
- `formatDuration`, `formatCost`, `elapsedSeconds` — unchanged
- Backend timestamps remain UTC ISO 8601 — no backend changes
- `ActiveScenario.svelte` — doesn't use `formatDateTime`
