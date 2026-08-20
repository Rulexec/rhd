# Testing Infrastructure

## Purpose
End-to-end testing for backend (Rust). Tests validate complete workflows: daemon startup, scenario execution, AI interactions, and WebSocket communication.

## Backend E2E Tests

### Test Runner: `rhd_test`
Orchestrates full integration tests:
- Starts mock OpenAI-compatible HTTP server
- Spawns `rhd daemon` with test configuration
- Runs `rhd run <scenario>` against daemon
- Validates AI request payloads (placeholder resolution, CWD propagation)
- Validates `rhd run` output and exit codes
- Cleans up daemon and temp directories

### Test Modes
- **Standard test:** Basic scenario execution with runCommand + aiChat + output
- **MCP test:** Scenario with MCP tool calls (built-in `rhd_set_flag`, skip conditions)

### CLI Arguments
```
--seed NUMBER          # RNG seed for deterministic tests (default: 42)
--repetitions NUMBER   # run tests N times with incrementing seed (default: 10)
```

### Mock AI Server
- HTTP server on random port
- `POST /v1/chat/completions` endpoint
- Records all requests for validation
- Returns configurable responses
- Supports streaming (SSE) with controlled chunk emission
- Control server endpoints for test coordination:
  - `/mock-response` — set AI response content
  - `/stream-chunk` — emit streaming chunk
  - `/stream-finish` — finish stream
  - `/stream-ready` — check stream state

### Test Scenarios
Located in `test_e2e/scenarios/`:
- `rhd_test/` — standard test scenario (runCommand → aiChat → output)
- `mcp_test/` — MCP tool test scenario (aiChat with tools → runCommand with skip)

### Deterministic Testing
- Seeded RNG for all random generation (AI responses, exit codes)
- Temp scripts pre-generated with deterministic output
- Same seed → same test behavior (reproducible failures)
- Per-repetition state: fresh daemon, new temp dirs, reset mock server

## Running Tests

### Mise Commands
```bash
mise run test-cargo              # cargo unit tests
mise run test-e2e                # backend E2E (cargo build + rhd_test)
mise run test-all                # all tests (cargo + e2e)
```

### Manual Execution
```bash
# Backend E2E
cargo build
cargo run -p rhd_test -- --seed 123 --repetitions 5

```
