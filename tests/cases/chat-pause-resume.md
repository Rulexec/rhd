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

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) - `pauses and resumes streaming chat` (pause steps 2-7, resume steps 2-7) - **SKIPPED**

### UI Tests
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders pause button when streaming and not paused` (pause step 1)
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when paused` (resume step 1)

### Coverage Notes
- E2E test is skipped (`it.skip`), so pause steps 2-7 and resume steps 2-7 are not actually covered
- Partial coverage: pause step 1, resume step 1 covered (14%)
