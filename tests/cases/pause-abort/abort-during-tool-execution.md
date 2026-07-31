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
18. System optimistically adds the message to pendingMessages store, rendered as a gray user message below the loader of the interrupted call
19. Daemon stores message in message queue

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
30. System keeps the gray pending message visible, now rendered above the loader of the new call
31. System receives `chatMessageAdded` with the confirmed user message
32. System removes the matching gray pending message, leaving exactly one visible copy

## Expected Results
- Abort: Non-finished tools return "Aborted" error, finished tools keep results, all results inserted
- Message queue: Message is shown exactly once, with a gray background meaning "not yet part of the chat"
- Resume: Queued messages sent, tool loop continues

## Covered By

### E2E Tests
- [`chat-state.test.ts`](../../../frontend/src/tests/e2e/chat-state.test.ts) - `aborts during tool execution and resumes with error results` (steps 2-32) - Line 602

### UI Tests
- [`MessageInput.test.ts`](../../../frontend/src/tests/ui/MessageInput.test.ts) - `renders abort button when streaming` (step 1) - Line 54
- [`MessageInput.test.ts`](../../../frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when aborted` (step 20) - Line 150
- [`Message.test.ts`](../../../frontend/src/tests/ui/Message.test.ts) - `renders pending messages as gray user messages` (step 18) - Line 142
- [`ToolCall.test.ts`](../../../frontend/src/tests/ui/ToolCall.test.ts) - `shows Aborted error for cancelled tool calls` (step 11) - Line 13
