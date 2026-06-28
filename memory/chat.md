# Chat Feature

## Chat Backend (`ChatManager`)

The `ChatManager` in `packages/rhd_app/src/chat.rs` handles all chat operations:

- **State**: Holds `Arc<ChatDb>` for persistence and `Mutex<HashMap<i64, CancellationToken>>` for tracking active streams per chat
- **`create_chat(title)`**: Creates new chat, returns chat_id
- **`list_chats()`**: Returns all chats sorted by updated_at DESC
- **`get_chat(id)`**: Returns chat info and all messages
- **`delete_chat(id)`**: Deletes chat and cascades to messages
- **`send_message(chat_id, content, model, models, event_sender)`**:
  - Validates chat exists and model is available
  - Adds user message to DB, emits `MessageAdded` event
  - Builds message history from DB, calls `chat_stream_cancellable()`
  - Accumulates streaming content, emits `StreamChunk` events
  - On success: adds assistant message to DB, emits `MessageAdded` and `StreamFinished` events
  - On abort/error: emits `StreamError` event
  - Returns assistant message_id on success
- **`edit_and_resend(message_id, new_content, model, models, event_sender)`**:
  - Validates message exists
  - Updates message content in DB, truncates subsequent messages
  - Emits `MessageAdded` event for updated message
  - Re-streams AI response (same flow as `send_message`)
- **`abort_chat(chat_id)`**: Cancels active stream token if present, returns true if aborted

## Chat Events (`ChatEvent` enum)

- `StreamChunk { chat_id, content }`: Incremental text from streaming
- `StreamFinished { chat_id, message_id, finish_reason }`: Stream completed successfully
- `StreamError { chat_id, error }`: Stream failed or aborted
- `MessageAdded { chat_id, message }`: Message persisted to DB (user or assistant)

## Chat Database

- Chat data persisted in SQLite database at `<dbDir>/chats.db` (default: `rhd_db/chats.db`)
- Two tables: `chats` and `messages` with foreign key relationship
- `chats` table: `id` (INTEGER PRIMARY KEY), `title` (TEXT), `created_at` (TEXT), `updated_at` (TEXT)
- `messages` table: `id` (INTEGER PRIMARY KEY), `chat_id` (INTEGER FK), `role` (TEXT), `content` (TEXT), `created_at` (TEXT)
- Index on `messages.chat_id` for faster retrieval
- CASCADE DELETE: deleting a chat removes all its messages
- `add_message()` automatically updates chat's `updated_at` timestamp
- `truncate_messages(chat_id, after_message_id)`: deletes messages with id > after_message_id (for edit-and-resend)
- WAL mode and foreign keys enabled
- Thread-safe via `Mutex<Connection>`

## Daemon Integration

- `ChatDb` initialized at `<dbDir>/chats.db` alongside `meta.db`
- `ChatManager` created with `Arc<ChatDb>`
- `broadcast::channel(100)` for chat events (separate from execution events)
- `DaemonState` holds `chat_db`, `chat_manager`, `chat_event_sender`
- WebSocket handler subscribes to both execution and chat event channels
