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
14. System optimistically adds the message to pendingMessages store, rendered as a gray user message below the loader of the interrupted call
15. Daemon stores message in message queue

### Resume Phase
16. User clicks Resume button
17. System dispatches `resumeChat` action
18. System sends resume request to daemon
19. Daemon processes pending tool calls (if any)
20. Daemon executes tools, sends results to AI
21. Daemon processes queued messages
22. Daemon appends queued messages to chat history
23. Daemon sends AI chat request with full context
24. Daemon transitions to Running state
25. System receives `chatResumed` action
26. System sets isPaused to false
27. System sets isStreaming to true
28. System keeps the gray pending message visible, now rendered above the loader of the new call
29. System receives `chatMessageAdded` with the confirmed user message
30. System removes the matching gray pending message, leaving exactly one visible copy

## Expected Results
- Pause: AI completes naturally, tool calls remembered but not executed
- Message queue: Message is shown exactly once, with a gray background meaning "not yet part of the chat"
- Ordering: while paused the loader precedes the pending message; after resume the pending message precedes the loader
- Resume: Pending tool calls executed, queued messages sent, tool loop continues

## Covered By

### E2E Tests
- [`chat-state.test.ts`](../../../frontend/src/tests/e2e/chat-state.test.ts) - `pauses during AI call and resumes with pending tool calls` (steps 2-30) - Line 403

### Storybook Tests
- [`pause-during-ai-call.spec.ts`](../../../frontend/tests/storybook/pause-during-ai-call.spec.ts) - `pauses during AI call and resumes with pending tool calls` (steps 1-30, incl. ordering and gray styling) - Line 7

### UI Tests
- [`MessageInput.test.ts`](../../../frontend/src/tests/ui/MessageInput.test.ts) - `renders pause button when streaming and not paused` (step 1) - Line 126
- [`MessageInput.test.ts`](../../../frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when paused` (step 16) - Line 138
- [`MessageInput.test.ts`](../../../frontend/src/tests/ui/MessageInput.test.ts) - `allows message input when paused` (step 10) - Line 162
- [`Message.test.ts`](../../../frontend/src/tests/ui/Message.test.ts) - `renders pending messages as gray user messages` (step 14) - Line 142
