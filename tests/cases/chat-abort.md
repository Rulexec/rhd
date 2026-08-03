> **DEPRECATED**: This test case has been superseded by the more detailed test cases in `tests/cases/pause-abort/`. See:
> - [`pause-during-ai-call.md`](pause-abort/pause-during-ai-call.md)
> - [`pause-during-tool-execution.md`](pause-abort/pause-during-tool-execution.md)
> - [`abort-during-ai-call.md`](pause-abort/abort-during-ai-call.md)
> - [`abort-during-tool-execution.md`](pause-abort/abort-during-tool-execution.md)
> - [`message-queue-during-pause-abort.md`](pause-abort/message-queue-during-pause-abort.md)

# Test Case: Abort Chat

## Description
User aborts active streaming response.

## Preconditions
- Chat exists and is selected
- Streaming in progress (isStreaming = true)

## Steps
1. User clicks Abort button
2. System dispatches `abortChat` action
3. System sends abort request to daemon
4. Daemon stops processing
5. System receives `chatStreamError` or `chatStreamFinished` action
6. System sets isStreaming to false
7. System clears streamingMessageId

## Expected Results
- Streaming stops
- isStreaming set to false
- Input field re-enabled
- Partial response (if any) remains in message

## Actions
- `abortChat` — dispatched when user clicks Abort
- `chatStreamError` or `chatStreamFinished` — received from WebSocket

## Covered By

### E2E Tests
- [`chat-state.test.ts`](../../frontend/src/tests/e2e/chat-state.test.ts) - `aborts streaming chat and updates state` (steps 2-3)

### UI Tests
- [`MessageInput.test.ts`](../../frontend/src/tests/ui/MessageInput.test.ts) - `renders abort button when streaming` (step 1)

### Coverage Notes
- Steps 4-7 require daemon to send `chatStreamError` or `chatStreamFinished` event, which is not simulated by the mock server
- Partial coverage: steps 1-3 covered (43%)
