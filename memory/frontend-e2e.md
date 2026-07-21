# Frontend E2E Testing

## Overview

Frontend E2E tests spawn daemon via `rhd_test frontend`. Located in `frontend/src/tests/e2e/`. Config: `frontend/vitest.config.e2e.ts`.

## Testing Paradigm

**Actions-based testing**: Tests use `dispatch()` to trigger actions. ActionDispatcher routes to processors or test overrides. Tests intercept actions at dispatcher level using `_testOverrideAction()`.

**UI tests**: Separate unit tests in `frontend/src/tests/ui/` verify component rendering with mocked stores.

## Test Infrastructure

Tests spawn `rhd_test frontend` which starts:
- Mock AI server on random port
- Control HTTP server on random port (for test coordination)
- rhd daemon with WebSocket server on random port

## Actions Layer

### Architecture

```
Component → dispatch(action) → ActionDispatcher
                                    ↓
                          Check override → If exists, call handler
                                    ↓
                          Call processor → Updates stores / calls external
```

### Key Files

- `frontend/src/lib/actions/types.ts` — Action type definitions
- `frontend/src/lib/actions/dispatcher.ts` — ActionDispatcher with `_testOverrideAction`
- `frontend/src/lib/actions/processors.ts` — Action processors (wrap chatWs)
- `frontend/src/lib/actions/index.ts` — Public exports

### Test Override Mechanism

```typescript
import { dispatch, _testOverrideAction, _testClearOverrides } from '../../lib/actions';

beforeEach(() => {
  _testClearOverrides();
});

it('intercepts action', async () => {
  _testOverrideAction('sendMessage', async (action) => {
    // Read payload
    console.log(action.payload.content);
    // Dispatch response actions
    await dispatch({ type: 'chatStreamChunk', payload: { content: 'AI response' } });
    await dispatch({ type: 'chatStreamFinished' });
  });

  await dispatch({ type: 'sendMessage', payload: { content: 'Hello', model: 'test_model' } });
});
```

### Action Types

User actions (from UI):
- `createChat` — Create new chat
- `selectChat` — Select chat
- `deleteChat` — Delete chat
- `sendMessage` — Send message
- `editMessage` — Edit message
- `abortChat` — Abort streaming
- `pauseChat` — Pause during tool loop
- `resumeChat` — Resume after pause
- `selectModel` — Select model

System actions (from WebSocket or tests):
- `chatStreamChunk` — Streaming content chunk
- `chatThinkingChunk` — Thinking content chunk
- `chatStreamFinished` — Streaming complete
- `chatStreamError` — Streaming error
- `chatMessageAdded` — Message added to chat
- `chatPaused` — Chat paused
- `chatResumed` — Chat resumed

## State Reset

`resetAllStores()` from `src/lib/chatStores.ts` resets all Svelte stores to initial state. Call in `beforeEach()` to ensure test isolation.

## Test Utilities

Located in `frontend/src/tests/testUtils.ts`:

| Function | Description |
|----------|-------------|
| `waitForWebSocket()` | Waits for daemon to be ready |
| `configureMock(content)` | Sets mock AI response |
| `getRecordedRequests()` | Fetches recorded AI requests |
| `emitStreamChunk(content)` | Emits a streaming chunk via control server |
| `finishStream()` | Finishes the stream via control server |
| `waitForStreamReady()` | Polls control server until stream is ready |

## Test Execution

- WebSocket port is dynamically set via `setWsPort()` and `connectWebSocket()` from `src/lib/ws.ts`
- Run command: `mise run test-frontend-e2e`

## Test Files

### E2E Tests (State-Based)
- `frontend/src/tests/e2e/chat-state.test.ts` — State logic testing using `dispatch()`

### UI Tests (Unit)
- `frontend/src/tests/ui/MessageInput.test.ts` — MessageInput component rendering
- `frontend/src/tests/ui/ChatList.test.ts` — ChatList component rendering
- `frontend/src/tests/ui/Message.test.ts` — Message component rendering

## Human-Readable Test Cases

Test cases documented in `tests/cases/*.md`:
- `chat-create.md` — Create new chat
- `chat-send-message.md` — Send message and receive response
- `chat-streaming.md` — Streaming chunks display
- `chat-edit-message.md` — Edit message
- `chat-abort.md` — Abort streaming
- `chat-pause-resume.md` — Pause/resume during tool loop
- `chat-delete.md` — Delete chat
- `chat-select-model.md` — Select model
- `roles-integration.md` — Role selection, switching, and conflict detection

Tests reference covered test cases in file header comments.
