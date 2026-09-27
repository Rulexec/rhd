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
`messages` history. Chats are therefore identified by content: every request computes a rolling
hash chain over its messages (`h_i = blake3(h_{i-1} || message_i)`), and the chain is classified
against each chat's **frontier** — the longest history that chat has ever registered:

| Incoming history | Classification |
|------------------|----------------|
| identical to any registered history | **retry** — same chat |
| extends a chat's frontier (append-only) | **continuation** — same chat |
| forks from an earlier point of a chat (subset, sub-chat, edited/regenerated resend) | **branch** — a new, independent chat row |
| shares no prefix with any chat | **new chat** |

Each prefix hash has a single owner (first registrant) and is never re-pointed, so chats cannot
steal each other's identity. Branches intentionally have **no lineage columns** — the
parent/child relationship remains derivable from `prefix_hashes` if ever needed.

Content-only identification has inherent limits:

- Two clients extending byte-identical histories concurrently are indistinguishable — the first
  extension claims the chat, the later diverger becomes a branch.
- A sub-chat's seed request that exactly matches its parent's registered history is attributed
  to the parent as a retry; all of the sub-chat's subsequent traffic lands in its own branched
  chat.
- A harness that **mutates earlier messages mid-chat** (e.g. rewrites the system prompt with
  changing context) breaks the chain and forks a new chat.

### Schema

Schema version 2 (stamped in `PRAGMA user_version`). Opening a database with a different version
fails at startup with instructions to delete or move the file — there is no in-place migration.

| Table | Contents |
|-------|----------|
| `chats` | `id`, `title` (first user message, truncated to 100 chars), `model`, `created_at`, `updated_at` |
| `prefix_hashes` | `hash` → `chat_id`, `len` (message count the hash covers); a chat's frontier is its `MAX(len)` row |
| `requests` | one row per logged request: `chat_id`, `ts`, `method`, `path`, `model`, `stream`, `status`, `duration_ms`, `error`, `response_assembled` |
| `raw` | `request_body` (original bytes **before** `extraBody` injection) and `response_body` (full raw response; for streaming, the concatenated SSE bytes) |
| `messages` | normalized per-chat conversation, one row per message: `seq`, `role`, `message_json` (full canonical message), `content`, `tool_calls`, `tool_call_id`, `name`, `source` (`history`/`response`), `request_id` |

`response_assembled` holds the assistant message reconstructed from the exchange: parsed SSE
deltas (content, tool calls, finish reason) for streams, or `choices[0].message` for JSON
responses. `NULL` when the response could not be parsed.

The `messages` table mirrors each chat's conversation. Request histories are diff-appended in
first-seen order (this is how tool **results** — `role=tool` messages — are captured, since they
only ever appear inside subsequent histories); assembled responses append the assistant turn at
its own sequence position. The request history is authoritative: when a resent history disagrees
with stored rows, everything from the divergence point is replaced. `message_json` stores the
canonical message with top-level `finish_reason` stripped (so SSE-assembled and history-delivered
forms of the same turn compare equal); the other columns are extracted projections — `tool_calls`
as a JSON array on assistant turns, `tool_call_id` on tool-result turns — so the tool pipeline is
queryable without parsing raw bodies.

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

-- The chat's conversation with tool calls and results
SELECT seq, role, content, tool_calls, tool_call_id, source
FROM messages WHERE chat_id = 42 ORDER BY seq;

-- Assistant tool calls with their results, joined
SELECT a.seq, json_extract(c.value, '$.function.name') AS tool,
       json_extract(c.value, '$.function.arguments') AS arguments,
       t.content AS result
FROM messages a,
     json_each(a.tool_calls) c
LEFT JOIN messages t ON t.chat_id = a.chat_id AND t.tool_call_id = json_extract(c.value, '$.id')
WHERE a.chat_id = 42 AND a.tool_calls IS NOT NULL
ORDER BY a.seq, c.key;

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

There is a small web viewer: [`frontend-proxy`](../../frontend-proxy/README.md). **Note:** it
predates schema version 2 and does not yet understand the `messages` table or the new chat
grouping — expect it to need an update before it works against current databases.

Notes:

- Every request stores its full raw body, so a chat with *N* turns costs O(N²) storage —
  fine for a debug tool; delete the database file to reset.
- Logging is best-effort: database failures are logged via `tracing` and never affect proxied
  requests.
- Classification, hash registration, request insertion, and history diffing happen in one
  transaction per request, so interleaved requests from parallel chats cannot misattribute.
- The logging database is fully independent of the main `rhd_db` chat storage.
- Multiple proxy processes may share one `logging.path`: connections use WAL with a busy
  timeout, so concurrent writers queue instead of failing, and chat grouping is shared across
  the proxies. Caveat: if two proxies receive the first message of the same new chat at the
  same instant, each may create its own chat row. The database must be on a local filesystem —
  WAL does not work over network mounts.

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
(`tests/logging_tests.rs`) cover chat classification (continuations, retries, sub-chat subsets
branching into separate chats, edited-history forks), raw + assembled storage for streaming and
non-streaming responses, tool-call and tool-result persistence to `messages`, upstream failure
capture, pre-injection request bodies, and the schema-version guard.
