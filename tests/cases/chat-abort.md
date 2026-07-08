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
- `frontend/src/tests/e2e/chat-state.test.ts` (state logic)
- `frontend/src/tests/ui/MessageInput.test.ts` (UI rendering)
