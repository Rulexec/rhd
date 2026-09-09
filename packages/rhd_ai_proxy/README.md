# rhd_ai_proxy

A small utility binary (not part of the core product): an OpenAI-compatible reverse proxy that
forwards requests to a target provider and injects per-model `extraBody` fields into completion
requests. Streaming (SSE) responses are piped through unchanged.

## What it does

1. Listens on `proxy.port` (bound to `127.0.0.1`).
2. For every incoming request:
   - Strips a leading `/v1` from the path and appends the remainder to `proxy.target.path`
     (so `http://localhost:1234/v1/chat/completions` → `https://example.org/raw/openrouter/v1/chat/completions`).
   - Forwards all headers except `host`, `content-length`, and hop-by-hop headers
     (`connection`, `keep-alive`, `proxy-connection`, `te`, `trailer`, `transfer-encoding`, `upgrade`).
     The `Host` header is derived from the target URL automatically.
   - If the path is a completions endpoint (`/chat/completions` or `/completions`), the body is a
     JSON object, and its `model` matches a key in `proxy.models`, the model's `extraBody` keys are
     merged into the **top level** of the request body.
3. Pipes the upstream response (status, headers, raw byte stream) back to the client, so SSE
   streaming works with no buffering. Upstream connection failures produce `502 Bad Gateway`.

## Configuration

```yaml
proxy:
  port: 1234
  target:
    path: https://example.org/raw/openrouter/v1
  models:
    "z-ai/glm-5.3":
      extraBody:
        provider:
          sort: throughput
          max_price: {"prompt": 1, "completion": 2}
```

- Parsing is strict: unknown fields are rejected, and keys are camelCase (`extraBody`).
- `target.path` must be an absolute `http(s)` URL.
- `models` is optional; model keys are matched exactly against the request body's `model` field.

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

## Logging

Per request (via `tracing`, level controlled by `RUST_LOG`, default `info`):

- `incoming request` — method and path.
- `applied model extraBody override` — model name and target URL, when an override was merged
  (`debug`-level `no extraBody override applied` otherwise).
- `upstream responded` — upstream status code; `upstream request failed` (error) on connect failures.

## Development

```sh
cargo test -p rhd_ai_proxy
```

Integration tests run the proxy against local echo/SSE upstreams and cover path mapping, `Host`
rewrite, `extraBody` injection, header passthrough, query preservation, incremental SSE streaming,
error passthrough, and `502` on unreachable upstream.
