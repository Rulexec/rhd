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
17. System optimistically adds the message to pendingMessages store, rendered as a gray user message below the loader of the interrupted call
18. Daemon stores message in message queue

### Resume Phase
19. User clicks Resume button
20. System dispatches `resumeChat` action
21. System sends resume request to daemon
22. Daemon processes queued messages
23. Daemon appends queued messages to chat history
24. Daemon sends AI chat request with full context
25. Daemon transitions to Running state
26. System receives `chatResumed` action
27. System sets isPaused to false
28. System sets isStreaming to true
29. System keeps the gray pending message visible, now rendered above the loader of the new call
30. System receives `chatMessageAdded` with the confirmed user message
31. System removes the matching gray pending message, leaving exactly one visible copy

## Expected Results
- Abort: AI request cancelled, partial response discarded, no message saved
- Message queue: Message is shown exactly once, with a gray background meaning "not yet part of the chat"
- Resume: Queued messages sent, no aborted AI call in message history

## Covered By

### E2E Tests
- [`chat-state.test.ts`](../../../frontend/src/tests/e2e/chat-state.test.ts) - `aborts during AI call and resumes without aborted message` (steps 2-31) - Line 531

### UI Tests
- [`MessageInput.test.ts`](../../../frontend/src/tests/ui/MessageInput.test.ts) - `renders abort button when streaming` (step 1) - Line 54
- [`MessageInput.test.ts`](../../../frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when aborted` (step 19) - Line 150
- [`Message.test.ts`](../../../frontend/src/tests/ui/Message.test.ts) - `renders pending messages as gray user messages` (step 17) - Line 142
