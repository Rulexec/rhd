# Test Case: Message Queue During Pause/Abort

## Description
User adds messages while chat is paused or aborted, messages are queued and sent on resume.

## Preconditions
- Chat exists and is selected
- Chat is paused or aborted (isPaused = true)

## Steps

### Queue First Message
1. User types first message in input field
2. User clicks Send button
3. System dispatches `queueMessage` action
4. System sends queue request to daemon
5. System optimistically adds the message to pendingMessages store, rendered as a gray user message
6. Daemon stores message in message queue

### Queue Second Message
7. User types second message in input field
8. User clicks Send button
9. System dispatches `queueMessage` action
10. System sends queue request to daemon
11. System optimistically adds the second message to pendingMessages store, rendered as a gray user message
12. Daemon stores message in message queue

### Resume
13. User clicks Resume button
14. System dispatches `resumeChat` action
15. System sends resume request to daemon
16. Daemon processes queued messages in order
17. Daemon appends first queued message to chat history
18. Daemon appends second queued message to chat history
19. Daemon sends AI chat request with full context
20. Daemon transitions to Running state
21. System receives `chatResumed` action
22. System sets isPaused to false
23. System sets isStreaming to true
24. System keeps both gray pending messages visible, still rendered below the loader, because the interrupted call has not produced its result yet
25. System receives `chatStreamFinished` while resumed and promotes both queued messages into the chat in queue order as regular user messages
26. System receives `chatMessageAdded` for each queued message in order, replacing each promoted message in place and leaving exactly one visible copy per message

## Expected Results
- Queue: Messages shown in order, each exactly once, with a gray background meaning "not yet part of the chat"
- Resume: Queued messages appended in order, AI call triggered with full context
- Pending messages are removed only as the daemon confirms each of them, not on resume

## Covered By

### E2E Tests
- [`chat-state.test.ts`](../../../frontend/src/tests/e2e/chat-state.test.ts) - `queues multiple messages during pause and sends on resume` (steps 3-26) - Line 691

### UI Tests
- [`MessageInput.test.ts`](../../../frontend/src/tests/ui/MessageInput.test.ts) - `allows message input when paused` (step 1) - Line 162
- [`Message.test.ts`](../../../frontend/src/tests/ui/Message.test.ts) - `renders pending messages as gray user messages` (steps 5, 11) - Line 142
- [`Message.test.ts`](../../../frontend/src/tests/ui/Message.test.ts) - `does not mark regular messages as pending` (step 26) - Line 158
