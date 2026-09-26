# rhd_ai_proxy

A small utility binary (not part of the core product): an OpenAI-compatible reverse proxy that
forwards requests to a target provider and injects per-model `extraBody` fields into completion
requests. Streaming (SSE) responses are piped through unchanged. An optional `logging` section
records every completions chat into a SQLite database for inspection and debugging.

## What it does

1. Listens on `proxy.port` (bound to `127.0.0.1`).
2. For every incoming request:
   - Strips a leading `/v1` from the path and appends the remainder to `proxy.target.path`
     (so `http://localhost:1234/v1/chat/completions` → `https://example.org/raw/openrouter/v1/chat/completions`).
   - Forwards all headers except `host`, `content-length`, and hop-by-hop headers
     (`connection`, `keep-alive`, `proxy-connection`, `te`, `trailer`, `transfer-encoding`, `upgrade`).
     The `Host` header is derived from the target URL automatically.
   - When `proxy.target.apiKey` is configured, an `Authorization: Bearer <token>` header is
     sent to the target on every request, replacing any `Authorization` header supplied by
     the client. When no `apiKey` is configured, the client's `Authorization` header is
     forwarded unchanged.
   - If the path is a completions endpoint (`/chat/completions` or `/completions`), the body is a
     JSON object, and its `model` matches a key in `proxy.models`, the model's `extraBody` keys are
     merged into the **top level** of the request body.
   - When `proxy.logging` is configured, completions requests are recorded into a SQLite
     database (see [Chat logging](#chat-logging)).
3. Pipes the upstream response (status, headers, raw byte stream) back to the client, so SSE
   streaming works with no buffering. Upstream connection failures produce `502 Bad Gateway`.

## Configuration

```yaml
proxy:
  port: 1234
  target:
    path: https://example.org/raw/openrouter/v1
    # Optional: sent as `Authorization: Bearer <token>` on every forwarded request.
    # Accepts a literal string or `{ env: "VAR_NAME" }` to read from the environment.
    apiKey: sk-example-token
  models:
    "z-ai/glm-5.3":
      extraBody:
        provider:
          sort: throughput
          max_price: {"prompt": 1, "completion": 2}
  # Optional: record every completions chat into a SQLite database.
  logging:
    path: ./proxy-logs
```

- Parsing is strict: unknown fields are rejected, and keys are camelCase (`extraBody`, `apiKey`).
- `target.path` must be an absolute `http(s)` URL.
- `target.apiKey` is optional. When set, it overrides any client-supplied `Authorization` header.
- `models` is optional; model keys are matched exactly against the request body's `model` field.
- `logging.path` is a folder for the database file (`chats.sqlite3`); relative paths are resolved
  against the config file location. The folder is created when missing. Omit the section to
  disable logging entirely.

See [`proxy.example.yaml`](proxy.example.yaml).

## Body injection example

Incoming:

```json
{"model": "z-ai/glm-5.3", "messages": [{"role": "user", "content": "Hello"}]}
```

Forwarded to target:

```json
{"model": "z-ai/glm-5.3", "messages": [{"role": "user", "content": "Hello"}],
 "provider": {"sort": "throughput", "max_price": {"prompt": 1, "completion": 2}}}
```

## Usage

```sh
cargo run -p rhd_ai_proxy -- --config packages/rhd_ai_proxy/proxy.example.yaml
```

Point any OpenAI-compatible client at `http://127.0.0.1:1234/v1`. For example, in `rhd.yaml`
model config use `baseUrl: http://127.0.0.1:1234/v1`.

```sh
curl -N http://127.0.0.1:1234/v1/chat/completions \
  -H 'content-type: application/json' \
  -d '{"model": "z-ai/glm-5.3", "messages": [{"role": "user", "content": "Hello"}], "stream": true}'
```

## Chat logging

With `proxy.logging` configured, the proxy records every completions request (any request whose
body is a JSON object with a non-empty `messages` array) into `chats.sqlite3` inside the given
folder. The goal is debugging: see which chats passed through and exactly what the harness sent
and received. Non-completions traffic (e.g. `GET /v1/models`, embeddings) is not logged.

### Chat identity

The OpenAI-compatible API is stateless — requests carry no chat identifier, only the full
`messages` history. Continuations are therefore detected by hashing that history: every request
computes a rolling hash chain over its messages (`h_i = blake3(h_{i-1} || message_i)`), and a
request whose history starts with a previously seen prefix continues that chat (longest prefix
wins).

- Append-only harnesses chain all turns of a conversation into one chat; an identical retry
  also maps to the same chat.
- An edited/branched history starts a new chat from the edit point.
- A harness that **mutates earlier messages mid-chat** (e.g. rewrites the system prompt with
  changing context) breaks the chain and starts a new chat — an inherent limitation of the
  stateless API.

### Schema

| Table | Contents |
|-------|----------|
| `chats` | `id`, `title` (first user message, truncated to 100 chars), `model`, `created_at`, `updated_at` |
| `prefix_hashes` | `hash` → `chat_id` map used for continuation detection |
| `requests` | one row per logged request: `chat_id`, `ts`, `method`, `path`, `model`, `stream`, `status`, `duration_ms`, `error`, `response_assembled` |
| `raw` | `request_body` (original bytes **before** `extraBody` injection) and `response_body` (full raw response; for streaming, the concatenated SSE bytes) |

`response_assembled` holds the assistant message reconstructed from the exchange: parsed SSE
deltas (content, tool calls, finish reason) for streams, or `choices[0].message` for JSON
responses. `NULL` when the response could not be parsed.

`status` is `NULL` while a response is still in flight; `error` is set when the proxy itself
failed to complete the exchange (e.g. upstream unreachable). If the client disconnects
mid-stream, the exchange is recorded with the bytes received so far.

### Inspecting

Use any SQLite client, e.g. `sqlite3 proxy-logs/chats.sqlite3`:

```sql
-- List chats, most recently active first
SELECT id, title, model, updated_at FROM chats ORDER BY updated_at DESC;

-- All requests of one chat, newest last
SELECT id, ts, model, stream, status, duration_ms, error
FROM requests WHERE chat_id = 42 ORDER BY id;

-- What the harness sent in the latest exchange of a chat (raw JSON)
SELECT request_body FROM raw
WHERE request_id = (SELECT MAX(id) FROM requests WHERE chat_id = 42);

-- Every raw request body of a chat, in order (each carries the full history)
SELECT r.id, r.ts, raw.request_body
FROM requests r JOIN raw ON raw.request_id = r.id
WHERE r.chat_id = 42 ORDER BY r.id;

-- The assembled assistant replies of a chat
SELECT id, response_assembled FROM requests
WHERE chat_id = 42 AND response_assembled IS NOT NULL ORDER BY id;

-- The raw streamed bytes of one exchange
SELECT response_body FROM raw WHERE request_id = 7;

-- Requests that never completed (proxy stopped or upstream failed)
SELECT * FROM requests WHERE status IS NULL OR error IS NOT NULL;
```

There is also a small web viewer: [`frontend-proxy`](../../frontend-proxy/README.md) renders
chats, reconstructed conversations, and raw request/response bodies in the browser. Point
`VITE_PROXY_LOGS_PATH` at the same folder as `proxy.logging.path` and run its dev server.

Notes:

- Every request stores its full raw body, so a chat with *N* turns costs O(N²) storage —
  fine for a debug tool; delete the database file to reset.
- Logging is best-effort: database failures are logged via `tracing` and never affect proxied
  requests.
- The logging database is fully independent of the main `rhd_db` chat storage.
- Multiple proxy processes may share one `logging.path`: connections use WAL with a busy
  timeout, so concurrent writers queue instead of failing, and chat grouping is shared across
  the proxies. Caveat: if two proxies receive the first message of the same new chat at the
  same instant, each may create its own chat row (the last prefix registration wins from then
  on). The database must be on a local filesystem — WAL does not work over network mounts.

## Logging

Per request (via `tracing`, level controlled by `RUST_LOG`, default `info`):

- `incoming request` — method and path.
- `applied model extraBody override` — model name and target URL, when an override was merged
  (`debug`-level `no extraBody override applied` otherwise).
- `upstream responded` — upstream status code; `upstream request failed` (error) on connect failures.
- `chat logging enabled` — database path, when `proxy.logging` is configured.

## Development

```sh
cargo test -p rhd_ai_proxy
```

Integration tests run the proxy against local echo/SSE upstreams and cover path mapping, `Host`
rewrite, `extraBody` injection, header passthrough, query preservation, incremental SSE streaming,
error passthrough, and `502` on unreachable upstream. Logging integration tests
(`tests/logging_tests.rs`) cover chat grouping by continuation, raw + assembled storage for
streaming and non-streaming responses, upstream failure capture, and pre-injection request
bodies.
