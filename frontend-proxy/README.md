# frontend-proxy

Read-only web viewer for `rhd_ai_proxy` chat logging. Shows which conversations passed through
the proxy, their reconstructed content, and the raw request/response bytes of every exchange.

Dev-only tool: the JSON API lives in the Vite dev server middleware; there is no production
build story. The viewer never writes to the logging database.

## Prerequisites

- Node v24 (see [.nvmrc](.nvmrc))
- A `chats.sqlite3` produced by `rhd_ai_proxy` with `proxy.logging` enabled (see
  [../packages/rhd_ai_proxy/README.md](../packages/rhd_ai_proxy/README.md), "Chat logging")

## Setup

`VITE_PROXY_LOGS_PATH` must point at the directory containing `chats.sqlite3` — the same folder
as `proxy.logging.path` in the proxy YAML. The dev server refuses to start without it (the error
names the variable and what it must point at); it also aborts with a hint when the directory
exists but holds no `chats.sqlite3` yet (run the proxy once with logging enabled first).

```sh
npm install

# from inside frontend-proxy/:
VITE_PROXY_LOGS_PATH=../packages/rhd_ai_proxy/proxy-logs npm run dev
# → http://localhost:5174 (strictPort — fails loudly if the port is taken)

# or from the repo root via mise (same command under the hood):
VITE_PROXY_LOGS_PATH=../packages/rhd_ai_proxy/proxy-logs mise run dev-frontend-proxy
```

Relative paths resolve against `frontend-proxy/` (npm runs the dev script there), so use a
`../`-prefixed path or an absolute path.

## Using

- Chats sidebar lists logged chats (title, model, relative time, request count), most recently
  active first.
- Click a chat: the conversation view — the latest request's message history plus one final
  assistant turn (markdown-rendered; ⏳ while the response is in flight, an error note when the
  exchange failed) — and the request timeline (time, model, SSE badge, status — HTTP code,
  ⏳ in-flight, ERR — and duration).
- Click a request row: inline drill-down with three tabs — **Raw Request** (original bytes
  before `extraBody` injection), **Raw Response** (verbatim SSE for streams), and **Assembled
  Reply** (the reconstructed assistant message).
- Refresh (header): re-fetches chats and the open selection; the selection is preserved and
  cleared only if it no longer exists after the refresh.

## API (dev middleware)

| Endpoint              | Returns                                              |
|-----------------------|------------------------------------------------------|
| GET /api/chats        | chat summaries with request counts, updated_at DESC  |
| GET /api/chats/:id    | chat + request summaries + derived conversation      |
| GET /api/requests/:id | request metadata + raw bodies + assembled reply      |

Only GET is served (405 otherwise); unknown ids → 404; failures → 500 with a JSON error
envelope. The Zod schemas in [src/lib/api/schemas.ts](src/lib/api/schemas.ts) are the single
contract shared by the middleware and the browser client.

## Notes

- Conversation reconstruction: each logged request body carries the harness's full message
  history, so the conversation view renders the **latest** request's history plus only the
  final assistant turn — append-only harnesses already embed earlier replies in that history.
  Per-request assembled replies stay reachable via each timeline row's drill-down.
- Raw bodies cross the API as UTF-8 text (lossy on invalid bytes) and can be large — the O(N²)
  storage note in the proxy README applies; the views render them as plain preformatted text.
- The database is opened in normal mode, not `readonly` (a WAL database cannot be opened
  read-only when no writer is attached — `-shm` recovery needs write access), with a 5 s busy
  timeout so concurrent proxy writes don't fail viewer reads. Read-only behavior is enforced by
  the query layer, which contains only `SELECT` statements; see
  [src/server/queries.ts](src/server/queries.ts).
- Assistant markdown is rendered with `marked` without a sanitizer — non-string assembled
  payloads fall back to HTML-escaped preformatted text. The tool trusts its own local logging
  database; do not point it at untrusted databases.

## Development

```sh
npm run check   # svelte-check
npm run test    # vitest (component tests jsdom, server tests node)
```

Or via mise from the repo root: `mise run check-frontend-proxy` / `mise run test-frontend-proxy`
(`mise run check` includes the svelte-check alongside the cargo checks).
