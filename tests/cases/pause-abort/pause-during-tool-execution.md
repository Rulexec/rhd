# Test Case: Pause During Tool Execution

## Description
User pauses chat while tool calls are executing.

## Preconditions
- Chat exists and is selected
- AI chat call completed with tool calls
- Tool calls are executing (isStreaming = true, isPaused = false)

## Steps

### Pause Phase
1. User clicks Pause button
2. System dispatches `pauseChat` action
3. System sends pause request to daemon
4. Daemon waits for current tool calls to complete
5. Daemon inserts tool results into chat messages
6. Daemon does NOT continue to next iteration
7. Daemon transitions to Paused state
8. System receives `chatPaused` action
9. System sets isPaused to true
10. System sets isStreaming to false

### Message Queue Phase
11. User types message in input field
12. User clicks Send button
13. System dispatches `queueMessage` action
14. System sends queue request to daemon
15. Daemon stores message in message queue
16. System receives `messageQueued` action
17. System adds message to queuedMessages store with "Queued" indicator

### Resume Phase
18. User clicks Resume button
19. System dispatches `resumeChat` action
20. System sends resume request to daemon
21. Daemon processes queued messages
22. Daemon appends queued messages to chat history
23. Daemon sends AI chat request with full context
24. Daemon transitions to Running state
25. System receives `chatResumed` action
26. System sets isPaused to false
27. System sets isStreaming to true
28. System clears queuedMessages store

## Expected Results
- Pause: Current tool calls complete, results inserted, tool loop paused
- Message queue: Messages stored with "Queued" indicator
- Resume: Queued messages sent, tool loop continues

## Covered By

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) - `pauses during tool execution and resumes` (steps 2-28) - Line 443

### UI Tests
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders pause button when streaming and not paused` (step 1) - Line 119
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when paused` (step 18) - Line 131
