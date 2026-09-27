# Proxy Logs Viewer (frontend-proxy)

Read-only Svelte 5 + MobX web app that visualizes the chat-logging SQLite database produced by
`rhd_ai_proxy`'s `proxy.logging` feature. Dev-only tool run via vite dev on port 5174
(`strictPort`). Sibling of `frontend/`, not a route inside it.

## What it does

- Chats sidebar: logged chats (title, model, relative time, request count), most recently
  active first.
- Chat detail: the chat's normalized conversation from the logging `messages` table (schema v2)
  — system/user/assistant/tool turns in seq order, assistant content markdown-rendered, tool
  calls with pretty-printed arguments, tool results with name/`tool_call_id` — plus a tail
  state for the latest request (⏳ in-flight, ERR failed). Every turn (including the tail) has a
  `raw #N` button opening the drill-down of its attributed exchange: the request whose history
  first carried the message, or that produced the response — so "what history was used" is one
  click away without scanning the raw-requests list.
- Index panel (right, always shown with a loaded chat): one-line entries for user messages and
  tool-less assistant responses; clicking scrolls the conversation to the turn and flashes it.
- Request timeline (below the conversation): HTTP status, duration, SSE badge — **collapsed by
  default** (toggle header; a `#N selected` hint appears while collapsed with a request open).
- Request drill-down: raw request body (pre-`extraBody`-injection), raw response body
  (verbatim SSE for streams), assembled assistant reply. A turn's `raw #N` button renders it
  inline right after that message (or after the pending/error tail) and auto-scrolls it into
  view; a second click on the same button or its × Close dismisses it. Timeline row clicks
  render it below the timeline (bottom placement); the timeline keeps its collapse state.
- Refresh button (header): re-fetches everything visible, preserving the selection; a selection
  that vanished after the refresh is cleared (a 404 mid-refresh counts as vanished, not as an
  error).

## Running

- `VITE_PROXY_LOGS_PATH` env var → directory containing `chats.sqlite3` (same semantics as
  `proxy.logging.path` in the proxy YAML). Relative paths resolve against `frontend-proxy/`.
- Fail-fast: dev server aborts at config load when the variable is unset (error names it),
  when the database file is missing (error says to run the proxy with logging enabled first),
  and when `PRAGMA user_version` ≠ 2 (error names the file and the delete-or-move remedy —
  matches the proxy's own schema guard).
- `mise run dev-frontend-proxy`, `mise run check-frontend-proxy`, `mise run test-frontend-proxy`
  (the check task is part of the repo-wide `mise run check` group).

## Architecture

- A Vite plugin (`src/server/plugin.ts`) installs a connect middleware on the dev server; it
  opens the SQLite database once via `better-sqlite3` in **normal** mode — a WAL database cannot
  be opened read-only when no writer is attached (`-shm` recovery needs write access) — with a
  5 s busy timeout so concurrent proxy writes don't fail viewer reads. Read-only is enforced by
  the query layer (`src/server/queries.ts` contains only `SELECT`s; no write routes exist,
  non-GET → 405).
- JSON API served by the middleware: `GET /api/chats`, `GET /api/chats/:id`,
  `GET /api/requests/:id`; unknown ids → 404, failures → 500 with a JSON error envelope. The
  Zod schemas in `src/lib/api/schemas.ts` are the single contract shared by server and client —
  shapes cannot drift.
- Client: one MobX root store `ProxyLogsStore` (`makeAutoObservable`, generator-method flows,
  public observable fields) plus a fetch-based API client with a DI interface (`ProxyLogsApi`)
  and context injection — the same conventions as the main frontend (see
  [MEMORY.md](MEMORY.md)): `mobxObservable` bridge, `$derived` bindings, component harness
  tests with a mocked store in Svelte context.

## Key decisions

- Separate sibling app, no shared npm package — small utils (`mobxObservable`, `yieldPromise`)
  are duplicated; extract only if a third consumer appears.
- Conversation = the `messages` table, not raw-body parsing. The proxy diff-appends request
  histories and appends assembled responses into normalized rows; the viewer maps them to turns
  in seq order and parses the projection columns (`content`, `tool_calls`) server-side, so tool
  calls/results are first-class without parsing raw blobs. A pending/error tail is appended for
  the latest request; a completed request adds no tail (its response row ends the conversation).
- Every assistant row with string content renders as markdown (not just the newest reply);
  non-string content renders as pretty JSON. Per-request assembled replies stay reachable via
  the timeline drill-down.
- Row classification: in-flight = `status NULL` **and** `error NULL`; check `error` first — a
  failed exchange also has `status NULL`.
- The `source` field (history/response) is served by the API but not rendered: it is constant
  (`history`) for system/user/tool rows and near-default (`response`) for assistant rows; the
  per-turn `raw #N` button carries the useful attribution instead.
- Timeline collapse is UI-local `$state` inside `RequestTimeline` (no props/events); opening a
  request from a turn button deliberately leaves it collapsed — the inline drill-down
  auto-scrolls into view.
- Drill-down placement uses an anchor model: the store keeps `openRequestAnchor` — the
  anchored turn's `seq`, `'tail'`, or `null` (bottom section below the timeline, used by
  timeline rows). Exactly one drill-down exists at a time; turn buttons toggle (same request
  already open at the same anchor closes it), and toggle identity is the turn, not the
  requestId — sibling turns sharing a requestId re-anchor instead of closing. If the anchored
  turn no longer exists after a refresh (e.g. the tail completed into a response row), the
  drill-down falls back to the bottom placement instead of vanishing. `openRequest` applies
  its fetched detail only when the selection still targets that request (stale-apply guard),
  so closing or re-targeting mid-flight cannot resurrect a zombie detail.
- Index panel + scroll orchestration live in `ChatDetailView`: turn anchors are
  `data-turn-seq` attributes, the flash is a `:global(.turn-flash)` class toggled imperatively
  (defined in `ConversationView` so it travels with turn styles), and all `scrollIntoView` calls
  go through a jsdom guard (`scrollIntoViewSafe`) so component tests exercise the wiring.
- Raw bodies cross the API as UTF-8 text (lossy on invalid bytes); JSON is pretty-printed
  client-side, SSE shown verbatim, both in height-capped `<pre>` blocks (bodies can be huge —
  O(N²) storage per chat by proxy design).
- Assistant markdown renders via `marked` without a sanitizer — the tool trusts its own local
  logging database; do not point it at untrusted databases.

## Where things live

- Server: `frontend-proxy/src/server/{index,plugin,db,queries,routes,fixture}.ts` (`fixture.ts`
  duplicates the proxy's schema DDL — provenance: `packages/rhd_ai_proxy/src/logging/schema.rs`
  — and stamps the same `user_version`; the viewer must not depend on the Rust crate)
- Contract: `frontend-proxy/src/lib/api/schemas.ts`
- Client API: `frontend-proxy/src/lib/api/{ProxyLogsApi,proxyLogsApiImpl}.ts`
- Store: `frontend-proxy/src/stores/ProxyLogsStore.ts`
- Components: `frontend-proxy/src/lib/components/*.svelte`

## Related

- [rhd_ai_proxy chat logging](../../packages/rhd_ai_proxy/README.md#chat-logging) — what gets
  logged and the SQLite schema
- [frontend-proxy README](../../frontend-proxy/README.md) — setup and usage
