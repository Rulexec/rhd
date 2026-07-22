# Test Case: Abort During Tool Execution

## Description
User aborts chat while tool calls are executing.

## Preconditions
- Chat exists and is selected
- AI chat call completed with tool calls
- Tool calls are executing (isStreaming = true, isPaused = false)

## Steps

### Abort Phase
1. User clicks Abort button
2. System dispatches `abortChat` action
3. System sends abort request to daemon
4. Daemon cancels execution of non-finished tool calls
5. Non-finished tool calls return "Aborted" error result
6. Finished tool calls keep their results
7. Daemon inserts all tool results (finished + aborted) into chat messages
8. Daemon transitions to Aborted state
9. Daemon emits `toolCallAborted` events for non-finished tools
10. System receives `toolCallAborted` actions
11. System updates tool call UI to show "Aborted" error
12. System sets isPaused to true
13. System sets isStreaming to false

### Message Queue Phase
14. User types message in input field
15. User clicks Send button
16. System dispatches `queueMessage` action
17. System sends queue request to daemon
18. Daemon stores message in message queue
19. System receives `messageQueued` action
20. System adds message to queuedMessages store with "Queued" indicator

### Resume Phase
21. User clicks Resume button
22. System dispatches `resumeChat` action
23. System sends resume request to daemon
24. Daemon processes queued messages
25. Daemon appends queued messages to chat history
26. Daemon sends AI chat request with full context
27. Daemon transitions to Running state
28. System receives `chatResumed` action
29. System sets isPaused to false
30. System sets isStreaming to true
31. System clears queuedMessages store

## Expected Results
- Abort: Non-finished tools return "Aborted" error, finished tools keep results, all results inserted
- Message queue: Messages stored with "Queued" indicator
- Resume: Queued messages sent, tool loop continues

## Covered By

### E2E Tests
- [`chat-state.test.ts`](../../../frontend/src/tests/e2e/chat-state.test.ts) - `aborts during tool execution and resumes with error results` (steps 2-31) - Line 539

### UI Tests
- [`MessageInput.test.ts`](../../../frontend/src/tests/ui/MessageInput.test.ts) - `renders abort button when streaming` (step 1) - Line 54
- [`MessageInput.test.ts`](../../../frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when aborted` (step 21) - Line 143
- [`ToolCall.test.ts`](../../../frontend/src/tests/ui/ToolCall.test.ts) - `shows Aborted error for cancelled tool calls` (step 11) - Line 13
