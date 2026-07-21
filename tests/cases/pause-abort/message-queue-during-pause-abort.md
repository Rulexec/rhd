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
5. Daemon stores message in message queue
6. System receives `messageQueued` action
7. System adds message to queuedMessages store with "Queued" indicator

### Queue Second Message
8. User types second message in input field
9. User clicks Send button
10. System dispatches `queueMessage` action
11. System sends queue request to daemon
12. Daemon stores message in message queue
13. System receives `messageQueued` action
14. System adds message to queuedMessages store with "Queued" indicator

### Resume
15. User clicks Resume button
16. System dispatches `resumeChat` action
17. System sends resume request to daemon
18. Daemon processes queued messages in order
19. Daemon appends first queued message to chat history
20. Daemon appends second queued message to chat history
21. Daemon sends AI chat request with full context
22. Daemon transitions to Running state
23. System receives `chatResumed` action
24. System sets isPaused to false
25. System sets isStreaming to true
26. System clears queuedMessages store

## Expected Results
- Queue: Messages stored in order with "Queued" indicator
- Resume: Queued messages appended in order, AI call triggered with full context

## Covered By

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) - `queues multiple messages during pause and sends on resume` (steps 3-26)

### UI Tests
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `allows message input when paused` (step 1)
- [`Message.test.ts`](frontend/src/tests/ui/Message.test.ts) - `shows Queued indicator for queued messages` (step 7)
