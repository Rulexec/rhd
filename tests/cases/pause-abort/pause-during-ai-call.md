# Test Case: Pause During AI Call

## Description
User pauses chat while AI is in thinking or streaming state.

## Preconditions
- Chat exists and is selected
- AI chat call in progress (isStreaming = true, isPaused = false)
- AI is in thinking or final message streaming state

## Steps

### Pause Phase
1. User clicks Pause button
2. System dispatches `pauseChat` action
3. System sends pause request to daemon
4. Daemon waits for AI call to complete naturally
5. If AI emits tool calls, daemon stores them in pending_tool_calls
6. Daemon transitions to Paused state
7. System receives `chatPaused` action
8. System sets isPaused to true
9. System sets isStreaming to false

### Message Queue Phase
10. User types message in input field
11. User clicks Send button
12. System dispatches `queueMessage` action
13. System sends queue request to daemon
14. Daemon stores message in message queue
15. System receives `messageQueued` action
16. System adds message to queuedMessages store with "Queued" indicator

### Resume Phase
17. User clicks Resume button
18. System dispatches `resumeChat` action
19. System sends resume request to daemon
20. Daemon processes pending tool calls (if any)
21. Daemon executes tools, sends results to AI
22. Daemon processes queued messages
23. Daemon appends queued messages to chat history
24. Daemon sends AI chat request with full context
25. Daemon transitions to Running state
26. System receives `chatResumed` action
27. System sets isPaused to false
28. System sets isStreaming to true
29. System clears queuedMessages store

## Expected Results
- Pause: AI completes naturally, tool calls remembered but not executed
- Message queue: Messages stored with "Queued" indicator
- Resume: Pending tool calls executed, queued messages sent, tool loop continues

## Covered By

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) - `pauses during AI call and resumes with pending tool calls` (steps 2-29) - Line 395

### UI Tests
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders pause button when streaming and not paused` (step 1) - Line 119
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when paused` (step 17) - Line 131
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `allows message input when paused` (step 10) - Line 155
