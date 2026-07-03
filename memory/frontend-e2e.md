# Frontend E2E Testing

## Overview

Frontend E2E tests spawn daemon via `rhd_test frontend`. Located in `frontend/src/tests/e2e/`. Config: `frontend/vitest.config.e2e.ts`.

## Test Infrastructure

Tests spawn `rhd_test frontend` which starts:
- Mock AI server on random port
- Control HTTP server on random port (for test coordination)
- rhd daemon with WebSocket server on random port

## Test Utilities

Located in `frontend/src/tests/testUtils.ts`:

| Function | Description |
|----------|-------------|
| `waitForWebSocket()` | Waits for daemon to be ready |
| `configureMock(content)` | Sets mock AI response |
| `getRecordedRequests()` | Fetches recorded AI requests |
| `emitStreamChunk(content)` | Emits a streaming chunk via control server (returns JSON with status) |
| `finishStream()` | Finishes the stream via control server (returns JSON with status) |
| `waitForStreamReady()` | Polls control server until stream is ready |

All control server methods check response status and throw errors if not OK.

## Test Execution

- Tests use `@testing-library/svelte` for component rendering and interaction
- WebSocket port is dynamically set via `setWsPort()` and `connectWebSocket()` from `src/lib/ws.ts`
- Run command: `mise run test-frontend-e2e`

## Known Issues

- Chat UI does not auto-select first model when creating new chat (test works around this)

## Test Files

- `frontend/src/tests/e2e/chat.test.ts` — Basic chat functionality
- `frontend/src/tests/e2e/chat-messageflow.test.ts` — Message flow testing
- `frontend/src/tests/e2e/chat-streaming.test.ts` — Streaming response testing
