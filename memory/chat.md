# Chat Feature

## ToolCall Structure

The `ToolCall` struct in `packages/rhd_ai/src/client.rs` matches OpenAI API format:
```rust
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}
```

Access tool name via `tool_call.function.name`, arguments via `tool_call.function.arguments`.

## Tool Loop Streaming

The `tool_loop()` in `packages/rhd_chat/src/tools.rs` streams the **final agent message** with tool call support:
1. Streaming `chat_stream_with_tools()` calls during tool loop (content and thinking streamed)
2. Tool calls are made globally unique using atomic counter
3. Event ordering: `ToolCallStarted` sent BEFORE `MessageAdded` (intermediate assistant) to ensure frontend creates temp message first
4. When final response received (no tool calls): emit `StreamFinished` when complete

Tool calls visible via `ToolCallStarted`/`ToolCallCompleted` events, with content streamed separately.

## Chat Backend (`ChatManager`)

The `ChatManager` in `packages/rhd_chat/src/manager.rs` handles all chat operations:

- **State**: Holds `Arc<ChatDb>` for persistence and `Mutex<HashMap<i64, CancellationToken>>` for tracking active streams per chat
- **`create_chat(title)`**: Creates new chat, returns chat_id
- **`list_chats()`**: Returns all chats sorted by updated_at DESC
- **`get_chat(id)`**: Returns chat info and all messages
- **`delete_chat(id)`**: Deletes chat and cascades to messages
- **`send_message(chat_id, content, model, models, event_sender)`**:
  - Validates chat exists and model is available
  - Adds user message to DB, emits `MessageAdded` event
  - Builds message history from DB, calls `chat_stream_cancellable()`
  - Accumulates streaming content, emits `StreamChunk` events (filters out empty content chunks)
  - On success: adds assistant message to DB, emits `MessageAdded` and `StreamFinished` events
  - On abort/error: emits `StreamError` event
  - Returns assistant message_id on success
- **`edit_and_resend(message_id, new_content, model, models, event_sender)`**:
  - Validates message exists
  - Updates message content in DB, truncates subsequent messages
  - Emits `MessageAdded` event for updated message
  - Re-streams AI response (same flow as `send_message`, filters out empty content chunks)
- **`abort_chat(chat_id)`**: Cancels active stream token if present, returns true if aborted
- **`attach_project(chat_id, project_name, event_sender)`**: Attaches project to chat, starts MCP clients, emits `ProjectAttached` event. Called synchronously from WebSocket handler (not spawned) to ensure errors propagate to frontend.

## Chat Events (`ChatEvent` enum)

- `StreamChunk { chat_id, content }`: Incremental text from streaming (empty content chunks are filtered out)
- `ThinkingChunk { chat_id, content }`: Incremental thinking/reasoning content from AI
- `StreamFinished { chat_id, message_id, finish_reason }`: Stream completed successfully
- `StreamError { chat_id, error }`: Stream failed or aborted
- `MessageAdded { chat_id, message }`: Message persisted to DB (user, assistant, or system)
- `ToolCallStarted { chat_id, tool_call_id, tool_name, arguments, mcp_name }`: MCP tool call started
- `ToolCallCompleted { chat_id, tool_call_id, result }`: MCP tool call completed
- `ChatPaused { chat_id }`: Chat paused during tool loop
- `ChatResumed { chat_id }`: Chat resumed from pause
- `DevNotification { title, message }`: Developer notification
- `ProjectAttached { chat_id, project_name }`: Project attached to chat
- `ProjectDetached { chat_id, project_name }`: Project detached from chat

## Chat Database

- Chat data persisted in SQLite database at `<dbDir>/chats.db` (default: `rhd_db/chats.db`)
- Two tables: `chats` and `messages` with foreign key relationship
- `chats` table: `id` (INTEGER PRIMARY KEY), `title` (TEXT), `created_at` (TEXT), `updated_at` (TEXT), `active_model` (TEXT, nullable)
- `messages` table: `id` (INTEGER PRIMARY KEY), `chat_id` (INTEGER FK), `role` (TEXT), `content` (TEXT), `created_at` (TEXT), `model` (TEXT, nullable), `thinking_content` (TEXT, nullable)
- Index on `messages.chat_id` for faster retrieval
- CASCADE DELETE: deleting a chat removes all its messages
- `add_message()` automatically updates chat's `updated_at` timestamp
- `truncate_messages(chat_id, after_message_id)`: deletes messages with id > after_message_id (for edit-and-resend)
- WAL mode and foreign keys enabled
- Thread-safe via `Mutex<Connection>`

### Model Tracking

- Each chat tracks its `active_model` (the currently selected model for that chat)
- Each message stores which `model` was used to generate it
- When sending a message, the chat's `active_model` is updated to match the model used
- Model indicators are shown in the UI when the model changes between messages (visual only, not sent to AI)

### Database Migration

- On initialization, `ChatDb` checks if the `active_model` column exists in the `chats` table
- If missing, it adds the column using `ALTER TABLE chats ADD COLUMN active_model TEXT`
- Similarly checks and adds the `model` column to the `messages` table if missing
- Migration is automatic and transparent to the application
- Existing data is preserved; new columns default to NULL for old records

## Daemon Integration

- `ChatDb` initialized at `<dbDir>/chats.db` alongside `meta.db`
- `ChatManager` created with `Arc<ChatDb>`
- `broadcast::channel(100)` for chat events (separate from execution events)
- `DaemonState` holds `chat_db`, `chat_manager`, `chat_event_sender`
- WebSocket handler subscribes to both execution and chat event channels

## Reload Interaction

Chat operations (`send_message`, `edit_and_resend`) acquire a read lock on `reload_lock` before starting. This ensures:
- If a reload is in progress, chat operations wait silently (no error returned)
- Reload waits for active chat streams to finish before proceeding
- New chat messages block until reload completes

## Chat Logging

When `logChats` is configured in `rhd.yaml`, the daemon writes detailed interaction logs for debugging:

- **Log directory**: `<logChats>/<sanitized-chat-title>-<YYYY-MM-DD-HH-MM-SS>/log.txt`
- **Collision handling**: Appends `-2`, `-3`, etc. if directory exists
- **Logged content**:
  - Stream start: model, available tools, full messages array sent to API
  - Assistant response: reasoning (if present), message content, finish_reason, token usage
  - Tool calls: tool name, call ID, arguments
  - Tool results: tool name, call ID, result content
  - Stream finished: finish_reason, duration
  - Stream error: error message

Logging is implemented in `packages/rhd_chat/src/chat_log.rs` via `ChatLogSink`.

### Raw Logging (`logChatsRaw`)

When `logChatsRaw: true` is set in `rhd.yaml` (requires `logChats` to be configured), the daemon writes raw API request/response data to `raw.txt` in the same log directory:

- **File**: `<logChats>/<sanitized-chat-title>-<YYYY-MM-DD-HH-MM-SS>/raw.txt`
- **Logged content**:
  - Full JSON request body sent to AI API (model, messages, tools)
  - Each SSE streaming chunk as received (with index), showing which chunks contain `reasoning_content` vs `content`
  - Full JSON response body for non-streaming requests
  - Detailed error information (HTTP status, response body)

Raw logging uses the `RawLogger` trait defined in `packages/rhd_ai/src/client.rs`, implemented by `RawChatLogSink` in `packages/rhd_chat/src/chat_log.rs`.
