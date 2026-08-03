# Test Case: Send Message

## Description
User sends a message in chat and receives AI response.

## Preconditions
- Chat exists and is selected
- Model is selected
- WebSocket connected

## Steps
1. User types message in input field
2. User clicks Send button (or presses Enter)
3. System dispatches `sendMessage` action with content and model
4. System adds user message to messages store
5. System sets isStreaming to true
6. System sends request to daemon
7. Daemon processes message and sends response
8. System receives `chatStreamChunk` actions
9. System creates optimistic assistant message on first chunk
10. System appends chunks to assistant message
11. System receives `chatStreamFinished` action
12. System sets isStreaming to false
13. System receives `chatMessageAdded` with real message
14. System replaces optimistic message with real message

## Expected Results
- User message appears in message list
- Streaming indicator shown during response
- Assistant message content displayed
- Input field re-enabled after completion
- Message has real numeric ID after stream finished

## Actions
- `sendMessage` — dispatched when user clicks Send
- `chatStreamChunk` — received from WebSocket (or dispatched by test)
- `chatStreamFinished` — received from WebSocket (or dispatched by test)
- `chatMessageAdded` — received from WebSocket (or dispatched by test)

## Covered By

### E2E Tests
- [`chat-state.test.ts`](../../frontend/src/tests/e2e/chat-state.test.ts) - `sends message and receives streaming response from daemon` (steps 3-14)
- [`chat-state.test.ts`](../../frontend/src/tests/e2e/chat-state.test.ts) - `handles multiple messages in sequence` (steps 3-14, multiple iterations)

### UI Tests
- [`MessageInput.test.ts`](../../frontend/src/tests/ui/MessageInput.test.ts) - `renders send button when not streaming` (step 2)
- [`MessageInput.test.ts`](../../frontend/src/tests/ui/MessageInput.test.ts) - `disables send button when no chat selected` (preconditions)
- [`MessageInput.test.ts`](../../frontend/src/tests/ui/MessageInput.test.ts) - `disables send button when no model selected` (preconditions)
- [`MessageInput.test.ts`](../../frontend/src/tests/ui/MessageInput.test.ts) - `shows error message when streamError is set` (steps 12-13 area)
- [`MessageInput.test.ts`](../../frontend/src/tests/ui/MessageInput.test.ts) - `renders model selector with available models` (preconditions)

### Coverage Notes
- Step 1 (user types message) is not explicitly tested
- Partial coverage: steps 2-14 covered (93%)
