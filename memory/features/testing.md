# Testing Infrastructure

## Purpose
End-to-end testing for both backend (Rust) and frontend (Svelte). Tests validate complete workflows: daemon startup, scenario execution, AI interactions, WebSocket communication, and UI behavior.

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
- **Frontend test:** Spawns daemon with WebSocket server for frontend E2E tests

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

## Frontend E2E Tests

### Test Framework
- **Vitest** with jsdom environment
- **@testing-library/svelte** for component rendering
- **@testing-library/jest-dom** for assertions

### Test Infrastructure
- `rhd_test frontend` starts daemon with WebSocket + mock AI + control server
- Daemon runs with `--db-dir` pointing to temp directory (no `rhd_db/` in project folder)
- Tests connect to real daemon WebSocket (no mocking)
- Control server configures mock AI responses
- Test utilities in `frontend/src/tests/testUtils.ts`:
  - `configureMock(content)` — set AI response
  - `emitStreamChunk(content)` — emit streaming chunk
  - `finishStream()` — finish stream
  - `waitForStreamReady()` — wait for stream state
  - `setAutoStream(enabled)` — toggle auto/manual streaming mode

### Test Categories
- **Unit tests:** `frontend/src/tests/*.test.ts` (pure component/utils, no daemon)
- **E2E tests:** `frontend/src/tests/e2e/*.test.ts` (spawn daemon via `rhd_test frontend`)

### Test Files
- `chat.test.ts` — chat creation, model selection
- `chat-messageflow.test.ts` — message send/receive, no duplication
- `chat-streaming.test.ts` — streaming display, abort, retry
- `chat-mcp-tools.test.ts` — chat with MCP tools, tool calls, streaming
- `scenarios.test.ts` — scenario list, active/finished states

### Vitest Configs
- `vitest.config.unit.ts` — unit tests (fast, no daemon)
- `vitest.config.e2e.ts` — E2E tests (longer timeout, daemon spawn)

## Running Tests

### Mise Commands
```bash
mise run test-cargo              # cargo unit tests
mise run test-e2e                # backend E2E (cargo build + rhd_test)
mise run test-frontend-unit      # frontend unit tests
mise run test-frontend-e2e       # frontend E2E tests
mise run test-all                # all tests
```

### Manual Execution
```bash
# Backend E2E
cargo build
cargo run -p rhd_test -- --seed 123 --repetitions 5

# Frontend E2E
cd frontend
npm run test:e2e

# Frontend unit
cd frontend
npm run test:unit
```
