# frontend-proxy: Conversation UX Improvements

## Goal

Make the proxy-logs viewer's chat detail page navigable for long conversations:

1. **Right index panel** (always shown next to the conversation): lists user messages and
   assistant responses that made no tool calls. Clicking an entry scrolls the conversation
   to that turn and flashes it.
2. **Requests timeline collapsed by default**: the raw-requests list no longer dominates the
   page; expand it via its header when needed. It stays present at the bottom.
3. **Per-message raw-request button**: every turn gets a small `raw #N` button that opens the
   request drill-down for the request the turn is attributed to — for `history`-source rows
   the request whose payload **first carried this message** (its raw request body *is* the
   history that was used), for `response` rows the request that produced the reply.
4. **Remove the `history`/`response` source badge** from all turns (rendering only). It is
   constant noise for system/user/tool rows (always `history`) and near-default for assistant
   rows; the `raw #N` button now carries the useful attribution.

Decisions confirmed with the user: drill-down opens **below, timeline stays collapsed** and
the view auto-scrolls to it; index entries are **role tag + one-line preview** (chats-sidebar
style); source badge **removed everywhere**.

## Scope / Non-goals

- Client-only changes: components + tests + docs. **No** schema, API, middleware, query, or
  store changes — `MessageTurn.requestId` and `ConversationTurn[]` already carry everything
  needed.
- The schema/API keep the `source` field untouched; only its rendering goes away.
- No scroll-spy / active-entry tracking in the index panel (click-to-jump only).

## Layout

```
.chat-detail
├─ header (title, model, timestamps)                — unchanged
└─ .chat-detail-body            (NEW: flex row, flex:1, min-height:0)
   ├─ .chat-detail-scroll       (existing scroll column, flex:1)
   │  ├─ ConversationView       (turns + raw #N buttons, no source badge)
   │  ├─ RequestTimeline        (collapsed by default, toggle header)
   │  └─ RequestDetailView      (when a request is open; auto-scrolled into view)
   └─ aside.conversation-index  (NEW: ~280px, border-left, own overflow-y)
      └─ ConversationIndex      (user + tool-less assistant entries)
```

## Implementation Steps

### 1. `RequestTimeline.svelte` — collapsible, collapsed by default

- Internal `let collapsed = $state(true)` (UI-local state; no new props/events).
- Header row becomes a `<button data-testid="requests-toggle">` spanning the section:
  `▸/▾ Requests (N)`, `aria-expanded`, keyboard-usable by virtue of being a button.
- Render the `<ul>` of rows only when expanded.
- When collapsed **and** `selectedRequestId !== null`, append a muted hint to the header
  (e.g. `· #123 selected`) so an open drill-down stays discoverable.
- Row rendering, statuses, selection, and the `requestSelect` event are unchanged.

### 2. `ConversationView.svelte` — turn buttons, seq anchors, badge removal

- Delete the `source-badge` span (+ its CSS and the `source` rendering). Field stays in the
  props type (schema unchanged).
- Add `data-turn-seq={turn.seq}` to message-turn containers (scroll targets).
- Add a small button in each turn's role row: label `raw #{requestId}`,
  `data-testid="turn-request-button"`, `title` explaining the semantics ("open the raw
  exchange this turn first appeared in / was produced by"). On click dispatch
  `requestSelect: { requestId }` via `createEventDispatcher` — same event shape
  `RequestTimeline` uses.
  - Present on **all** turns: message turns and the pending/error tails (both carry
    `requestId`; investigating the in-flight/failed exchange is exactly the debugging case).
- Component stays presentational (no store access).
- Add `:global(.turn-flash)` keyframe animation (background pulse, ~1s) in the style block —
  the class is toggled from the outside (ChatDetailView), hence `:global`.

### 3. New `ConversationIndex.svelte` — right panel

- Props: `conversation: ConversationTurn[]`. Presentational, dispatcher
  `entrySelect: { seq: number }`.
- Entries derived from message turns where:
  - `role === 'user'`, or
  - `role === 'assistant'` **and** no tool calls (`toolCalls === null || toolCalls.length === 0`).
  System, tool, and tool-calling assistant turns are excluded (per requirements).
- Entry layout: small role tag (`user` / `assistant`, muted or colored) + single-line
  truncated preview (`commonStyles['truncate']`), matching the chats sidebar look.
  `data-testid="index-entry"`.
- Preview extraction helper (defensive, mirrors `ConversationView` style):
  string content → as-is; array of parts → concatenate `part.text` values; anything else →
  compact `JSON.stringify` or `—` when empty.
- Header `Index (N)`; empty state ("No user or assistant messages") when no entries match.
- The panel itself doesn't scroll internally per-entry; its list container gets
  `overflow-y: auto` so long conversations don't push the layout.

### 4. `ChatDetailView.svelte` — layout + scroll orchestration

- Wrap the existing scroll column in the new `.chat-detail-body` flex row; append
  `<aside>` with `<ConversationIndex conversation={detail.conversation} />` (rendered in the
  loaded-detail branch only — loading/error states keep the full-width treatment).
- `bind:this` on the scroll container; helpers:
  - `scrollToTurn(seq)`: `querySelector('[data-turn-seq="…"]')` → guarded
    `scrollIntoView({ behavior: 'smooth', block: 'start' })` (guard `typeof … === 'function'`
    for jsdom) + add `.turn-flash`, remove after ~1.2s timeout.
  - `scrollToRequestDetail()`: after the open resolves and `tick()`, scroll
    `[data-testid="request-detail"]` into view (same guard).
- `handleRequestSelect` (now serving both timeline rows and turn buttons): await
  `flowResult(store.openRequest(id))` (swallow errors — store already surfaces them), then
  `tick()`, then `scrollToRequestDetail()`. Timeline is **not** force-expanded.
- Index entry clicks call `scrollToTurn(seq)`.
- Store untouched.

### 5. Tests (harness + testing-library patterns, mocked store in context)

- **`ConversationIndex.test.ts` (new)**:
  - renders entries for user and tool-less assistant turns only (system / tool /
    assistant-with-tool-calls excluded);
  - preview from string content and from parts-array content;
  - click dispatches `entrySelect` with the turn's `seq`;
  - empty state when nothing matches.
- **`ConversationView.test.ts` (update)**: replace the source-badge test with "no source
  badge rendered"; every message turn + pending/error tail renders `turn-request-button`
  with the right `raw #N` label; click dispatches `requestSelect { requestId }`;
  `data-turn-seq` anchors present.
- **`RequestTimeline.test.ts` (update)**: existing tests expand the timeline first (or assert
  against the expanded state after clicking the toggle); new cases — collapsed by default
  (no rows in DOM), toggle expands/collapses with `aria-expanded`, collapsed header shows
  the selected-request hint.
- **`ChatDetailView.test.ts` (update)**: timeline rows hidden until toggled; index panel
  rendered with entries; clicking a turn's `raw #N` button calls `openRequest(requestId)`;
  request-detail rendering/close behavior unchanged.

### 6. Validation

```sh
mise run check-frontend-proxy
mise run test-frontend-proxy
```

### 7. Docs

- `frontend-proxy/README.md` — "Using" section: describe the index panel, collapsed
  timeline, per-turn raw-request button; drop the source-badge mention.
- `memory/frontend/proxy-logs-viewer.md` — "What it does" + "Key decisions": same updates,
  and note that `source` is still served by the API but no longer rendered.

## Risks / Notes

- `scrollIntoView` is not implemented in jsdom — all scroll helpers are guarded so tests
  exercise the wiring (openRequest calls, DOM queries) without crashing.
- The `:global(.turn-flash)` class is applied imperatively from ChatDetailView; keep the
  animation definition next to turn styles in ConversationView so it travels with them.
- All files stay well under the 500-line limit (ConversationView grows by ~40 lines).
- The drill-down's default tab remains "Raw Request", which is exactly the "what history was
  used" view for `raw #N` clicks — no tab logic changes needed.
