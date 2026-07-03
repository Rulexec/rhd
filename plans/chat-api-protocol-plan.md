# Chat API Protocol Plan

## Goal

Extend WebSocket protocol in `rhd_api` with chat-related request types and streaming event types.

## Scope

- New `WsRequest` variants for chat CRUD and messaging
- New event types for streaming responses
- New error codes for chat-specific errors
- All types serializable via serde (JSON over WebSocket)

## Request Types

```rust
pub enum WsRequest {
    // ... existing variants ...

    #[serde(rename = "createChat")]
    CreateChat { id: String, title: String },

    #[serde(rename = "listChats")]
    ListChats { id: String },

    #[serde(rename = "getChat")]
    GetChat { id: String, chat_id: i64 },

    #[serde(rename = "deleteChat")]
    DeleteChat { id: String, chat_id: i64 },

    #[serde(rename = "sendMessage")]
    SendMessage {
        id: String,
        chat_id: i64,
        content: String,
        model: String,
    },

    #[serde(rename = "editMessage")]
    EditMessage {
        id: String,
        message_id: i64,
        content: String,
        model: String,
    },

    #[serde(rename = "abortChat")]
    AbortChat { id: String, chat_id: i64 },
}
```

## Event Types

Streaming events use existing `WsEvent` structure with new event names:

```rust
// Event names (sent via WsEvent.event field):
// "chatStreamChunk" — partial content from AI
// "chatStreamFinished" — AI response complete
// "chatStreamError" — AI request failed
// "chatMessageAdded" — new message persisted (user or assistant)
// "chatUpdated" — chat metadata changed (title, etc.)

// Payloads (sent via WsEvent.data field):

// chatStreamChunk
{
    "chatId": 1,
    "content": "Hello"
}

// chatStreamFinished
{
    "chatId": 1,
    "messageId": 42,
    "finishReason": "stop"
}

// chatStreamError
{
    "chatId": 1,
    "error": "API error: 429 Too Many Requests"
}

// chatMessageAdded
{
    "chatId": 1,
    "message": {
        "id": 42,
        "chatId": 1,
        "role": "assistant",
        "content": "Hello!",
        "createdAt": "2026-06-28T15:00:00Z"
    }
}

// chatUpdated (for title changes, etc.)
{
    "chatId": 1,
    "title": "New title"
}
```

## Response Types

All requests return `WsResponse` with `data` field containing:

```rust
// createChat response
{ "chatId": 1 }

// listChats response
{
    "chats": [
        { "id": 1, "title": "...", "createdAt": "...", "updatedAt": "..." }
    ]
}

// getChat response
{
    "chat": { "id": 1, "title": "...", "createdAt": "...", "updatedAt": "..." },
    "messages": [
        { "id": 1, "chatId": 1, "role": "user", "content": "...", "createdAt": "..." }
    ]
}

// deleteChat response
{ "deleted": true }

// sendMessage response (immediate ack, streaming follows via events)
{ "messageId": 1 }

// editMessage response (immediate ack, streaming follows via events)
{ "messageId": 1 }

// abortChat response
{ "aborted": true }
```

## Error Codes

```rust
pub enum ErrorCode {
    // ... existing variants ...
    ChatNotFound,
    MessageNotFound,
    ChatStreamFailed,
}
```

## File Changes

| File | Change |
|------|--------|
| `packages/rhd_api/src/lib.rs` | Add `WsRequest` variants, `ErrorCode` variants, event payload structs |

## Implementation Steps

1. Add new `WsRequest` variants with serde rename attributes
2. Add new `ErrorCode` variants: `ChatNotFound`, `MessageNotFound`, `ChatStreamFailed`
3. Add event payload structs (optional, can use `serde_json::Value` inline):
   - `ChatStreamChunkEvent { chat_id: i64, content: String }`
   - `ChatStreamFinishedEvent { chat_id: i64, message_id: i64, finish_reason: String }`
   - `ChatStreamErrorEvent { chat_id: i64, error: String }`
   - `ChatMessageAddedEvent { chat_id: i64, message: ChatMessageDto }`
   - `ChatMessageDto { id: i64, chat_id: i64, role: String, content: String, created_at: String }`
4. Ensure all types derive `Serialize`, `Deserialize`, `Debug`, `Clone`
5. Add unit tests for serialization/deserialization

## Design Notes

- Event names follow existing pattern: lowercase, no separators (e.g., `chatstreamchunk`)
- Actually, looking at existing code: `WsEvent::new("scenariostarted", ...)` — event names are lowercase concatenated
- So chat events: `chatstreamchunk`, `chatstreamfinished`, `chatstreamerror`, `chatmessageadded`
- `id` field in requests is client-generated request ID for matching responses (existing pattern)
- Streaming events do not have request ID — they are broadcast to all subscribers
- `sendMessage` and `editMessage` return immediate ack with message ID, then stream events follow

## Success Criteria

- [ ] All new `WsRequest` variants serialize/deserialize correctly
- [ ] New `ErrorCode` variants work
- [ ] Event payload structs serialize correctly
- [ ] Backward compatibility: existing requests still work
- [ ] Unit tests pass

## Dependencies

None — standalone contract. Backend and frontend plans depend on this.
