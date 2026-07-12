# Test Case: Pause/Resume Chat

## Description
User pauses and resumes chat during tool loop execution.

## Preconditions
- Chat exists and is selected
- Tool loop running (isStreaming = true, isPaused = false)

## Pause Steps
1. User clicks Pause button
2. System dispatches `pauseChat` action
3. System sends pause request to daemon
4. Daemon pauses execution
5. System receives `chatPaused` action
6. System sets isPaused to true
7. System sets isStreaming to false

## Resume Steps
1. User clicks Resume button
2. System dispatches `resumeChat` action
3. System sends resume request to daemon
4. Daemon resumes execution
5. System receives `chatResumed` action
6. System sets isPaused to false
7. System sets isStreaming to true

## Expected Results
- Pause: isPaused true, isStreaming false, Resume button visible
- Resume: isPaused false, isStreaming true, Pause button visible

## Actions
- `pauseChat` — dispatched when user clicks Pause
- `resumeChat` — dispatched when user clicks Resume
- `chatPaused` — received from WebSocket
- `chatResumed` — received from WebSocket

## Covered By
- `frontend/src/tests/e2e/chat-state.test.ts` (state logic)
- `frontend/src/tests/ui/MessageInput.test.ts` (UI rendering)
