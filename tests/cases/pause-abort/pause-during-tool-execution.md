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
15. System optimistically adds the message to pendingMessages store, rendered as a gray user message below the loader of the interrupted call
16. Daemon stores message in message queue

### Resume Phase
17. User clicks Resume button
18. System dispatches `resumeChat` action
19. System sends resume request to daemon
20. Daemon processes queued messages
21. Daemon appends queued messages to chat history
22. Daemon sends AI chat request with full context
23. Daemon transitions to Running state
24. System receives `chatResumed` action
25. System sets isPaused to false
26. System sets isStreaming to true
27. System keeps the gray pending message visible, now rendered above the loader of the new call
28. System receives `chatMessageAdded` with the confirmed user message
29. System removes the matching gray pending message, leaving exactly one visible copy

## Expected Results
- Pause: Current tool calls complete, results inserted, tool loop paused
- Message queue: Message is shown exactly once, with a gray background meaning "not yet part of the chat"
- Resume: Queued messages sent, tool loop continues

## Covered By

### E2E Tests
- [`chat-state.test.ts`](../../../frontend/src/tests/e2e/chat-state.test.ts) - `pauses during tool execution and resumes` (steps 2-29) - Line 462

### UI Tests
- [`MessageInput.test.ts`](../../../frontend/src/tests/ui/MessageInput.test.ts) - `renders pause button when streaming and not paused` (step 1) - Line 126
- [`MessageInput.test.ts`](../../../frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when paused` (step 17) - Line 138
- [`Message.test.ts`](../../../frontend/src/tests/ui/Message.test.ts) - `renders pending messages as gray user messages` (step 15) - Line 142
