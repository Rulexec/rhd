# Frontend E2E Testing

## Overview

Frontend E2E tests spawn daemon via `rhd_test frontend`. Located in `frontend/src/tests/e2e/`. Config: `frontend/vitest.config.e2e.ts`.

## Testing Paradigm

**State-based testing** (preferred): Tests call state manipulation functions directly without rendering UI components. Verifies state changes and actual WebSocket communication with daemon.

**UI-based testing** (legacy): Tests render components and interact with DOM. Being phased out in favor of state-based approach.

## Test Infrastructure

Tests spawn `rhd_test frontend` which starts:
- Mock AI server on random port
- Control HTTP server on random port (for test coordination)
- rhd daemon with WebSocket server on random port

## State-Based Testing

### Test Exports

State manipulation functions are exported with `_test_` prefix from `src/lib/chatWs.ts`:

- `_test_createChat(title)` — Creates chat via daemon
- `_test_sendMessage(content, model)` — Sends message via daemon
- `_test_loadAvailableModels()` — Loads models from daemon
- `_test_selectChat(chatId)` — Selects chat
- `_test_deleteChat(chatId)` — Deletes chat
- `_test_editMessage(messageId, content, model)` — Edits message
- `_test_abortChat()` — Aborts streaming
- `_test_handleChatEvent(event, data)` — Handles chat events

### State Reset

`resetAllStores()` from `src/lib/chatStores.ts` resets all Svelte stores to initial state. Call in `beforeEach()` to ensure test isolation.

### Example State-Based Test

```typescript
import { _test_createChat, _test_sendMessage } from '../../lib/chatWs';
import { chats, messages, isStreaming, resetAllStores } from '../../lib/chatStores';

beforeEach(() => {
  resetAllStores();
});

it('creates chat and sends message', async () => {
  await _test_createChat('Test');
  await configureMock('AI response');
  await _test_sendMessage('Hello', 'test_model');
  
  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  });
  
  expect(get(messages).length).toBe(2);
});
```

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

- WebSocket port is dynamically set via `setWsPort()` and `connectWebSocket()` from `src/lib/ws.ts`
- Run command: `mise run test-frontend-e2e`

## Test Files

### State-Based Tests
- `frontend/src/tests/e2e/chat-state.test.ts` — State logic testing (chat creation, message flow, streaming)

### UI-Based Tests (Legacy)
- `frontend/src/tests/e2e/chat.test.ts` — Basic chat functionality with UI rendering
- `frontend/src/tests/e2e/chat-messageflow.test.ts` — Message flow with UI rendering
- `frontend/src/tests/e2e/chat-streaming.test.ts` — Streaming with UI rendering

## Known Issues

- Chat UI does not auto-select first model when creating new chat (test works around this)
