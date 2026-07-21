# Test Case: Abort During AI Call

## Description
User aborts chat while AI is in thinking or streaming state.

## Preconditions
- Chat exists and is selected
- AI chat call in progress (isStreaming = true, isPaused = false)
- AI is in thinking or final message streaming state

## Steps

### Abort Phase
1. User clicks Abort button
2. System dispatches `abortChat` action
3. System sends abort request to daemon
4. Daemon cancels current AI request immediately
5. Daemon discards partial response
6. If AI emitted tool calls, daemon does NOT execute them
7. Daemon transitions to Aborted state
8. Daemon emits `streamAborted` event
9. System receives `streamAborted` action
10. System removes streaming message from UI
11. System sets isPaused to true
12. System sets isStreaming to false

### Message Queue Phase
13. User types message in input field
14. User clicks Send button
15. System dispatches `queueMessage` action
16. System sends queue request to daemon
17. Daemon stores message in message queue
18. System receives `messageQueued` action
19. System adds message to queuedMessages store with "Queued" indicator

### Resume Phase
20. User clicks Resume button
21. System dispatches `resumeChat` action
22. System sends resume request to daemon
23. Daemon processes queued messages
24. Daemon appends queued messages to chat history
25. Daemon sends AI chat request with full context
26. Daemon transitions to Running state
27. System receives `chatResumed` action
28. System sets isPaused to false
29. System sets isStreaming to true
30. System clears queuedMessages store

## Expected Results
- Abort: AI request cancelled, partial response discarded, no message saved
- Message queue: Messages stored with "Queued" indicator
- Resume: Queued messages sent, no aborted AI call in message history

## Covered By

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) - `aborts during AI call and resumes without aborted message` (steps 2-30) - Line 491

### UI Tests
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders abort button when streaming` (step 1) - Line 54
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when aborted` (step 20) - Line 143
- [`Message.test.ts`](frontend/src/tests/ui/Message.test.ts) - `shows Queued indicator for queued messages` (step 10) - Line 140
