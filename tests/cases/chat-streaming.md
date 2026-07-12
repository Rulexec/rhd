# Test Case: Chat Streaming

## Description
System displays streaming chunks in real-time with animated indicator.

## Preconditions
- Chat exists and is selected
- Message sent, streaming in progress

## Steps
1. System receives `chatStreamChunk` action
2. If first chunk:
   - System creates optimistic assistant message with temp ID
   - System sets streamingMessageId to temp ID
   - System adds message to messages store
3. If subsequent chunk:
   - System appends content to message with streamingMessageId
4. System displays animated dots indicator while streaming
5. System receives `chatStreamFinished` action
6. System sets isStreaming to false
7. System receives `chatMessageAdded` with real message
8. System replaces optimistic message with real message
9. System clears streamingMessageId

## Expected Results
- Loader shown before first chunk
- Streaming message created on first chunk
- Content appended on subsequent chunks
- Animated dots visible during streaming
- Streaming indicator hidden after finish
- Message has real numeric ID after stream finished

## Actions
- `chatStreamChunk` — received from WebSocket (or dispatched by test)
- `chatStreamFinished` — received from WebSocket (or dispatched by test)
- `chatMessageAdded` — received from WebSocket (or dispatched by test)

## Covered By
- `frontend/src/tests/e2e/chat-state.test.ts` (state logic)
- `frontend/src/tests/ui/MessageList.test.ts` (UI rendering)
