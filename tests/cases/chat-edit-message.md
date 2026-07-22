# Test Case: Edit Message

## Description
User edits a user message, truncates subsequent messages, and re-streams.

## Preconditions
- Chat exists and is selected
- Messages exist in conversation
- Not currently streaming

## Steps
1. User clicks edit button on user message
2. System enters edit mode for that message
3. User modifies message content
4. User clicks Save (or presses Enter)
5. System dispatches `editMessage` action with messageId, new content, model
6. System updates message content in messages store
7. System truncates all messages after edited message
8. System sets isStreaming to true
9. System sends request to daemon
10. Daemon re-processes from edited message
11. System receives streaming response (same as send-message flow)

## Expected Results
- Message content updated
- Subsequent messages removed
- Streaming indicator shown
- New response received from edited point

## Actions
- `editMessage` — dispatched when user saves edit

## Covered By

### E2E Tests
- [`chat-state.test.ts`](../../frontend/src/tests/e2e/chat-state.test.ts) - `edits message and re-streams response` (steps 5-11)

### UI Tests
- [`Message.test.ts`](../../frontend/src/tests/ui/Message.test.ts) - `renders edit button for user messages` (step 1)

### Coverage Notes
- Steps 2-4 (edit mode UI) are not covered by tests
- Partial coverage: steps 1, 5-11 covered (73%)
