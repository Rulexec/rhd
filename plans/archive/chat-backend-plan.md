# Chat Backend Plan

## Goal

Implement `ChatManager` in `rhd_app`, integrate `ChatDb` into daemon state, and add WebSocket handlers for chat operations with streaming support.

## Scope

- `ChatManager` struct orchestrating chat operations
- Daemon state integration (`ChatDb` and `ChatManager` in `DaemonState`)
- WebSocket handlers for all chat requests
- Streaming AI responses broadcast to WebSocket clients
- Abort support via `CancellationToken`

## Architecture

```
WebSocket Client
       ↓ WsRequest
   ws.rs (handler)
       ↓
   ChatManager
       ↓
   ┌───┴───┬───────────┐
   ↓       ↓           ↓
ChatDb  OpenAiClient  CancellationToken
```

## ChatManager API

```rust
pub struct ChatManager {
    db: Arc<ChatDb>,
    active_streams: Mutex<HashMap<i64, CancellationToken>>,
}

pub enum ChatError {
    Db(DbError),
    Ai(AiError),
    ChatNotFound,
    MessageNotFound,
    ModelNotFound(String),
}

impl ChatManager {
    pub fn new(db: Arc<ChatDb>) -> Self;

    // CRUD
    pub fn create_chat(&self, title: &str) -> Result<i64, ChatError>;
    pub fn list_chats(&self) -> Result<Vec<ChatInfo>, ChatError>;
    pub fn get_chat(&self, id: i64) -> Result<Option<(ChatInfo, Vec<Message>)>, ChatError>;
    pub fn delete_chat(&self, id: i64) -> Result<(), ChatError>;

    // Messaging (async, streaming)
    pub async fn send_message(
        &self,
        chat_id: i64,
        content: String,
        model: &str,
        models: &HashMap<String, ModelConfig>,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<i64, ChatError>;

    pub async fn edit_and_resend(
        &self,
        message_id: i64,
        new_content: String,
        model: &str,
        models: &HashMap<String, ModelConfig>,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<i64, ChatError>;

    pub fn abort_chat(&self, chat_id: i64) -> bool;
}

// Events broadcast to WebSocket clients
pub enum ChatEvent {
    StreamChunk { chat_id: i64, content: String },
    StreamFinished { chat_id: i64, message_id: i64, finish_reason: String },
    StreamError { chat_id: i64, error: String },
    MessageAdded { chat_id: i64, message: Message },
}
```

## Daemon Integration

```rust
// packages/rhd_app/src/daemon.rs
pub struct DaemonState {
    // ... existing fields ...
    pub chat_db: Arc<ChatDb>,
    pub chat_manager: Arc<ChatManager>,
    pub chat_event_sender: broadcast::Sender<ChatEvent>,
}

// In run_daemon():
let chat_db_path = format!("{}/chats.db", db_dir);
let chat_db = Arc::new(ChatDb::new(&chat_db_path)?);
let chat_manager = Arc::new(ChatManager::new(chat_db.clone()));
let (chat_event_sender, _) = broadcast::channel(100);
```

## WebSocket Handler Updates

```rust
// packages/rhd_app/src/ws.rs

// In handle_ws_connection():
// - Subscribe to chat_event_sender
// - Forward ChatEvent → WsEvent to client

// In handle_ws_message():
// - Match new WsRequest variants
// - Call ChatManager methods
// - Return WsResponse

async fn handle_ws_connection(stream, state) {
    let mut events_rx = state.execution_tracker.subscribe();
    let mut chat_events_rx = state.chat_event_sender.subscribe();

    loop {
        tokio::select! {
            msg = read.next() => { /* existing + new chat handlers */ }
            event = events_rx.recv() => { /* existing execution events */ }
            chat_event = chat_events_rx.recv() => {
                // Convert ChatEvent → WsEvent and send
            }
        }
    }
}
```

## Send Message Flow

```rust
pub async fn send_message(&self, chat_id, content, model, models, event_sender) {
    // 1. Validate chat exists
    // 2. Validate model exists
    // 3. Add user message to DB
    // 4. Broadcast MessageAdded event
    // 5. Get all messages for chat
    // 6. Convert to ChatMessage vec
    // 7. Create CancellationToken, store in active_streams
    // 8. Call OpenAiClient::chat_stream_cancellable()
    // 9. For each chunk: broadcast StreamChunk event
    // 10. On success: add assistant message to DB, broadcast StreamFinished
    // 11. On error: broadcast StreamError
    // 12. On abort: broadcast StreamError with "aborted" message
    // 13. Remove cancellation token from active_streams
}
```

## Edit and Resend Flow

```rust
pub async fn edit_and_resend(&self, message_id, new_content, model, models, event_sender) {
    // 1. Get message to find chat_id
    // 2. Update message content in DB
    // 3. Truncate messages after message_id
    // 4. Broadcast MessageAdded event for updated message
    // 5. Get all messages for chat
    // 6. Stream AI response (same as send_message steps 6-13)
}
```

## File Changes

| File | Change |
|------|--------|
| `packages/rhd_app/src/chat.rs` | **NEW** — `ChatManager`, `ChatError`, `ChatEvent` |
| `packages/rhd_app/src/daemon.rs` | Add `chat_db`, `chat_manager`, `chat_event_sender` to `DaemonState`; initialize in `run_daemon()` |
| `packages/rhd_app/src/ws.rs` | Add chat request handlers; subscribe to `chat_event_sender`; forward `ChatEvent` as `WsEvent` |
| `packages/rhd_app/src/lib.rs` | Export `chat` module |
| `packages/rhd_app/Cargo.toml` | Add `tokio-util` dependency (for `CancellationToken`) if not present |

## Implementation Steps

1. Create `chat.rs` with `ChatManager` struct and `ChatError` enum
2. Implement CRUD methods (create_chat, list_chats, get_chat, delete_chat)
3. Implement `send_message()`:
   - Validate chat and model
   - Add user message to DB
   - Build message history
   - Stream AI response with cancellation token
   - Broadcast events
   - Save assistant message on success
4. Implement `edit_and_resend()`:
   - Update message, truncate, resend
5. Implement `abort_chat()`:
   - Cancel token for chat_id
6. Update `DaemonState` to include chat fields
7. Update `run_daemon()` to initialize chat DB and manager
8. Update `ws.rs`:
   - Add match arms for new `WsRequest` variants
   - Subscribe to `chat_event_sender`
   - Convert `ChatEvent` to `WsEvent` and send to client
9. Add error handling for all chat operations
10. Test with WebSocket client

## Design Notes

- `active_streams` map stores `CancellationToken` per chat_id — only one active stream per chat
- If `send_message` called while stream active, cancel previous stream first
- `ChatEvent` is separate from `ExecutionEvent` — different broadcast channel
- WebSocket handler subscribes to both event channels
- Model resolution: use `models` HashMap to get `ModelConfig`, create `OpenAiClient` per request
- Message history: convert `Message` (DB) → `ChatMessage` (AI client) for API call
- System prompt: not supported in first iteration (can add later)

## Success Criteria

- [ ] `ChatManager` CRUD operations work
- [ ] `send_message()` streams response and broadcasts events
- [ ] `edit_and_resend()` truncates and resends
- [ ] `abort_chat()` cancels active stream
- [ ] Daemon initializes chat DB at `rhd_db/chats.db`
- [ ] WebSocket handlers process all chat requests
- [ ] Chat events broadcast to all connected clients
- [ ] Error states handled correctly (chat not found, model not found, AI error)
- [ ] Only one active stream per chat

## Dependencies

- [chat-db-plan.md](chat-db-plan.md) — `ChatDb` implementation
- [chat-ai-streaming-plan.md](chat-ai-streaming-plan.md) — `OpenAiClient::chat_stream_cancellable()`
- [chat-api-protocol-plan.md](chat-api-protocol-plan.md) — `WsRequest` and event types
