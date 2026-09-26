# Proxy Logs Viewer (frontend-proxy)

Read-only Svelte 5 + MobX web app that visualizes the chat-logging SQLite database produced by
`rhd_ai_proxy`'s `proxy.logging` feature. Dev-only tool run via vite dev on port 5174
(`strictPort`). Sibling of `frontend/`, not a route inside it.

## What it does

- Chats sidebar: logged chats (title, model, relative time, request count), most recently
  active first.
- Chat detail: reconstructed conversation (latest request's message history + one final
  assistant turn) and a request timeline (HTTP status, ⏳ in-flight, ERR failed, SSE badge,
  duration).
- Request drill-down (timeline row click): raw request body (pre-`extraBody`-injection), raw
  response body (verbatim SSE for streams), assembled assistant reply.
- Refresh button (header): re-fetches everything visible, preserving the selection; a selection
  that vanished after the refresh is cleared (a 404 mid-refresh counts as vanished, not as an
  error).

## Running

- `VITE_PROXY_LOGS_PATH` env var → directory containing `chats.sqlite3` (same semantics as
  `proxy.logging.path` in the proxy YAML). Relative paths resolve against `frontend-proxy/`.
- Fail-fast: dev server aborts at config load when the variable is unset (error names it) and
  when the database file is missing (error says to run the proxy with logging enabled first).
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
- Conversation = the latest request's message history + **one** final assistant turn.
  Append-only harnesses embed earlier assistant replies in each request's history, so the
  latest history already is the full conversation; interleaving all assembled replies would
  duplicate them. Per-request replies stay reachable via the timeline drill-down.
- Row classification: in-flight = `status NULL` **and** `error NULL`; check `error` first — a
  failed exchange also has `status NULL`.
- Raw bodies cross the API as UTF-8 text (lossy on invalid bytes); JSON is pretty-printed
  client-side, SSE shown verbatim, both in height-capped `<pre>` blocks (bodies can be huge —
  O(N²) storage per chat by proxy design).
- Assistant markdown renders via `marked` without a sanitizer — the tool trusts its own local
  logging database; do not point it at untrusted databases.

## Where things live

- Server: `frontend-proxy/src/server/{index,plugin,db,queries,routes,fixture}.ts` (`fixture.ts`
  duplicates the proxy's schema DDL for tests — the viewer must not depend on the Rust crate)
- Contract: `frontend-proxy/src/lib/api/schemas.ts`
- Client API: `frontend-proxy/src/lib/api/{ProxyLogsApi,proxyLogsApiImpl}.ts`
- Store: `frontend-proxy/src/stores/ProxyLogsStore.ts`
- Components: `frontend-proxy/src/lib/components/*.svelte`

## Related

- [rhd_ai_proxy chat logging](../../packages/rhd_ai_proxy/README.md#chat-logging) — what gets
  logged and the SQLite schema
- [frontend-proxy README](../../frontend-proxy/README.md) — setup and usage
