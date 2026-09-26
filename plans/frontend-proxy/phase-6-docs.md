# Phase 6: Documentation and Memory Updates

## Overview

Make the finished viewer discoverable and documented, and record the new component in the
project knowledge base:

1. **`frontend-proxy/README.md`** — how to run the tool, env var semantics, port, endpoints,
   read-only guarantee, testing.
2. **`packages/rhd_ai_proxy/README.md`** — a pointer in the chat-logging "Inspecting" section to
   the GUI alternative.
3. **`memory/frontend/proxy-logs-viewer.md`** — new knowledge file describing the app's
   architecture and conventions.
4. **`memory/MEMORY.md`** — index row + project-tree sketch entry.
5. **`memory/frontend/MEMORY.md`** — related-docs pointer.

No code changes. Everything here describes behavior shipped in Phases 1–5; write it against the
final implementation (verify claims by running the app once before writing).

## Files to Create

### 1. `frontend-proxy/README.md`

```markdown
# frontend-proxy

Read-only web viewer for `rhd_ai_proxy` chat logging. Shows which conversations passed through
the proxy, their reconstructed content, and the raw request/response bytes of every exchange.

Dev-only tool: the JSON API lives in the Vite dev server middleware; there is no production
build story. The viewer never writes to the logging database.

## Prerequisites

- Node v24 (see .nvmrc)
- A `chats.sqlite3` produced by `rhd_ai_proxy` with `proxy.logging` enabled (see
  ../packages/rhd_ai_proxy/README.md, "Chat logging")

## Setup

VITE_PROXY_LOGS_PATH must point at the directory containing chats.sqlite3 — the same folder as
proxy.logging.path in the proxy YAML. The dev server refuses to start without it.

    npm install
    VITE_PROXY_LOGS_PATH=../packages/rhd_ai_proxy/proxy-logs npm run dev
    # → http://localhost:5174 (strictPort)

or via mise:

    mise run dev-frontend-proxy

## Using

- Chats sidebar lists logged chats, most recently active first.
- Click a chat: conversation view (latest request's history + final assistant reply) and the
  request timeline (status, duration, SSE badge, in-flight/errored states).
- Click a request row: raw request body (pre-extraBody-injection), raw response body (verbatim
  SSE for streams), and the assembled assistant message.
- Refresh (header): re-fetches chats and the open selection; the selection is preserved and
  cleared only if it no longer exists.

## API (dev middleware)

| Endpoint              | Returns                                              |
|-----------------------|------------------------------------------------------|
| GET /api/chats        | chat summaries with request counts, updated_at DESC  |
| GET /api/chats/:id    | chat + request summaries + derived conversation      |
| GET /api/requests/:id | request metadata + raw bodies + assembled reply      |

Only GET is served; unknown ids → 404; failures → 500, JSON error envelope.

## Notes

- Raw bodies are UTF-8 text (lossy on invalid bytes) and can be large — the O(N²) storage note
  in the proxy README applies.
- The database is opened in normal mode (WAL read-only open fails without a writer attached) but
  the query layer is SELECT-only by construction; see src/server/queries.ts.
- Assistant markdown is rendered with marked without sanitization — the tool trusts its own
  local logging database; do not point it at untrusted databases.

## Development

    npm run check   # svelte-check
    npm run test    # vitest (component tests jsdom, server tests node)
```

(Adjust wording to match what was actually built — e.g. exact placeholder texts, any sanitizer
decision from Phase 5.)

### 2. `memory/frontend/proxy-logs-viewer.md`

Product/infrastructure-view knowledge file — no SQL, no function bodies. Structure:

```markdown
# Proxy Logs Viewer (frontend-proxy)

Read-only Svelte 5 + MobX web app that visualizes the chat-logging SQLite database produced by
rhd_ai_proxy's proxy.logging feature. Dev-only tool run via vite dev on port 5174.

## What it does
- chats list / conversation view / request timeline / raw bodies (product behavior summary)
- refresh button semantics: re-fetch visible data, preserve selection

## Running
- VITE_PROXY_LOGS_PATH env var → directory containing chats.sqlite3 (same as proxy.logging.path)
- fail-fast when unset; strictPort 5174; mise run dev-frontend-proxy

## Architecture
- Vite plugin (src/server/plugin.ts) installs a connect middleware on the dev server; opens the
  SQLite DB via better-sqlite3 (normal mode — WAL read-only open fails without a writer; SELECT-
  only query layer enforces read-only)
- JSON API: /api/chats, /api/chats/:id, /api/requests/:id; Zod schemas in src/lib/api/schemas.ts
  are the single contract shared by server and client
- Client: single ProxyLogsStore (MobX, generator-method flows), fetch-based ProxyLogsApi with DI
  interface, same conventions as the main frontend (mobxObservable bridge, context injection,
  component harness tests)

## Key decisions
- separate sibling app, no shared package (duplicate small utils instead)
- conversation = latest request's message history + one final assistant turn (append-only
  harnesses embed earlier replies in history; interleaving all assembled replies duplicates)
- in-flight = status NULL and error NULL; check error first when classifying rows

## Where things live
- server: frontend-proxy/src/server/{plugin,db,queries,routes,fixture}.ts
- contract: frontend-proxy/src/lib/api/schemas.ts
- store: frontend-proxy/src/stores/ProxyLogsStore.ts
- components: frontend-proxy/src/lib/components/*.svelte
```

## Files to Modify

### 3. `packages/rhd_ai_proxy/README.md`

In the **"Inspecting"** section of "Chat logging" (after the sqlite3 code block, before the
"Notes:" list), insert a short pointer paragraph:

```markdown
There is also a small web viewer: [`frontend-proxy`](../../frontend-proxy/README.md) renders
chats, reconstructed conversations, and raw request/response bodies in the browser. Point
`VITE_PROXY_LOGS_PATH` at the same folder as `proxy.logging.path` and run its dev server.
```

Do not change anything else in the proxy README (no proxy behavior changed in this milestone).

### 4. `memory/MEMORY.md`

Two edits:

**a)** Project tree sketch (the `frontend/` line area, around the `frontend/` and `templates/`
entries): add a sibling entry right after `frontend/`:

```
├── frontend-proxy/         # Dev-only Svelte viewer for rhd_ai_proxy chat-logging SQLite DB
```

**b)** Knowledge Base Index table: add a row pointing to the new file:

```markdown
| [frontend/proxy-logs-viewer.md](frontend/proxy-logs-viewer.md) | When working on frontend-proxy, the proxy-logs viewer, its vite middleware SQLite API, or VITE_PROXY_LOGS_PATH |
```

### 5. `memory/frontend/MEMORY.md`

In the "Related Documentation" list at the end, append:

```markdown
- [Proxy Logs Viewer](proxy-logs-viewer.md)
```

## Tests / Verification

Docs phase — verification is editorial:

1. Every command in `frontend-proxy/README.md` works copy-paste: run `mise run
   dev-frontend-proxy` and the two `npm run` commands once each.
2. The env-var fail-fast message in the README matches the actual error text from
   `resolveLogsDir` (Phase 2).
3. Endpoint table matches `matchApiRoute` routes.
4. Memory files: `memory/MEMORY.md` links resolve; the tree sketch matches reality
   (`ls` the repo root); `memory/frontend/proxy-logs-viewer.md` contains no implementation
   snippets (product/infrastructure view only).
5. The proxy README pointer link resolves from `packages/rhd_ai_proxy/` (`../../frontend-proxy/
   README.md`).

## Implementation Notes

1. **Verify before writing:** boot the full stack once (proxy + viewer) and walk through a chat;
   the README's UI description must match shipped placeholder texts and behaviors, not the plan's
   intentions.
2. **Memory conventions:** per `memory/MEMORY.md`, features files stay product-view; the new
   memory file deliberately mixes a short product summary with an architecture map because this
   app *is* infrastructure — keep implementation detail (SQL, signatures) out.
3. **No plan archival here:** moving `plans/frontend-proxy-logs-viewer-grand-plan.md` and the
   phase files to `plans/archive/` is the job of the plans-archive skill after implementation is
   accepted — do not do it in this phase.

## Dependencies

- **Depends on:** Phases 2 (API surface documented), 4 (UI composition), 5 (final behavior).
  Drafting can start after Phase 4; finalize after Phase 5.
- **Blocks:** nothing — final phase.

## Verification Summary

```sh
mise run dev-frontend-proxy        # README quickstart works
npm --prefix frontend-proxy run check && npm --prefix frontend-proxy run test
mise run check                     # whole-repo check still green
```
