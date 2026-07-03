# Backend E2E Testing

## Overview

Backend E2E tests via `rhd_test` crate. Run command: `mise run test-e2e`.

## CLI Arguments

| Argument | Default | Description |
|----------|---------|-------------|
| `--seed` | 42 | Base seed for deterministic random generation |
| `--repetitions` | 10 | Number of test loop iterations |

Each iteration uses seed `base_seed + i`, prints iteration seed for reproducibility on failure.

Pass arguments: `mise run test-e2e -- --seed 100 --repetitions 5`

## Architecture

`rhd_test` starts:
1. Mock OpenAI-compatible HTTP server (axum, reused across iterations)
2. Spawns daemon per iteration
3. Runs scenario, validates AI request payloads and output

## Mock Server

### Controlled Streaming

- `StreamChunkSender` type: `Arc<Mutex<Option<mpsc::Sender<Option<String>>>>>`
- When streaming request arrives, creates channel and stores sender
- Control server endpoints send chunks via the sender
- SSE stream uses `async_stream::stream!` with keep-alive (1s interval)
- Control server returns JSON responses: `{"status":"OK"}` or `{"status":"ERROR","message":"..."}`

### Control Server

HTTP server on random port for test coordination. Endpoints:
- Configure mock responses
- Send stream chunks
- Finish streams
- Query recorded requests

## Test Data

- Test scenarios: `test_e2e/scenarios/<name>/scenario.yaml`
- Test models: `test_e2e/models/*.yaml`

## rhd_test Source Files

| File | Description |
|------|-------------|
| `main.rs` | E2E test runner: mock AI server, daemon spawn, validation |
| `args.rs` | CLI argument definitions |
| `mock_server.rs` | Mock OpenAI-compatible server with token usage |
| `control_server.rs` | HTTP control server for test coordination |
| `standard_test.rs` | Standard test with meta.json validation |
| `mcp_test.rs` | MCP tool test |
| `sse_test.rs` | SSE streaming test |
| `frontend_test.rs` | Frontend E2E test orchestrator |
| `utils.rs` | Test utilities |
