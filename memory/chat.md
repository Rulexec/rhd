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

The tool loop is now implemented using an FSM-driven architecture (see [fsm.md](fsm.md) for details). The `tool_loop()` function in `packages/rhd_chat/src/tools/tool_loop.rs` delegates to `FsmToolLoop` which drives the `ToolLoopFsm` state machine.

**Key components**:
- `ToolLoopFsm`: Synchronous state machine managing the tool loop lifecycle
- `FsmToolLoop`: Async wrapper that drives the FSM and handles I/O (AI calls, tool executions)
- `BuiltinFsmManager`: Coordinates helper FSMs for built-in tools
- `DB Sync Listener`: Synchronizes FSM state to database via event listeners

**Streaming behavior**:
1. Streaming `chat_stream_with_tools()` calls during tool loop (content and thinking streamed)
2. Tool calls are made globally unique using FSM's `tool_call_id_counter`
3. Event ordering: `ToolCallStarted` sent BEFORE `MessageAdded` (intermediate assistant) to ensure WebSocket client creates temp message first
4. When final response received (no tool calls): emit `StreamFinished` when complete

Tool calls visible via `ToolCallStarted`/`ToolCallCompleted` events, with content streamed separately.

## Todo List Injection

After each tool loop iteration, the current todo list is injected as a system message into the conversation context. This ensures the AI is aware of its task progress during multi-step operations.

**Implementation** (`packages/rhd_chat/src/tools.rs`):
- `inject_todo_list_message()`: Injects todo list as system message after tool results
- `render_environment_details_for_injection()`: Renders environment details with todo list and role
- Called at the end of each tool loop iteration (after tool results are processed)
- Uses `TemplateLoaderRef` to load templates from `templates/` directory
- Returns error if required templates are missing (no fallback values)

**Injection flow**:
1. Tool loop iteration completes (all tool results processed)
2. `inject_todo_list_message()` is called
3. Gets current todo list from database
4. Parses todo list into `TodoItem` structs
5. Renders environment details using templates
6. Adds as system message to database (persisted for AI context)
7. Next iteration starts with updated context

**Templates used**:
- `todo_list_empty.md`: Prompt when no todo list exists
- `todo_list_with_items.md`: Table format for todo items
- `environment_details_with_role.md`: Environment details with active role
- `environment_details_no_role.md`: Environment details without role

## Built-in Tools

### rhd_set_todo_list
- **Purpose**: Allows AI to manage a task tracking list during multi-step operations
- **Tool name**: `rhd_set_todo_list` (not namespaced)
- **Parameters**: `todos` (string) - Full markdown checklist
- **Behavior**: Replaces entire todo list with new one (no merge)
- **Storage**: Saved to database, persists across tool calls
- **Event**: Emits `TodoListUpdated` event for WebSocket client updates
- **Injection**: After each tool loop iteration, current todo list is injected as system message
- **Contract**: Tool contract injected as system message on first message in chat
- **Checkbox syntax**: `[ ]` (pending), `[-]` (in progress), `[x]` (completed), `[!]` (discarded)

### rhd_set_role
- **Purpose**: Switch the current active role for a chat
- **Tool name**: `rhd_set_role` (not namespaced)
- **Parameters**: `role_name` (string) - Name of the role to switch to
- **Behavior**: Sets active role, marks role prompt as pending injection
- **Event**: Emits `RoleChanged` event
- **Availability**: Only available when chat has attached projects with roles

## Template System

- Templates stored in `templates/` directory at project root
- Loaded at daemon startup by `TemplateLoader` in `packages/rhd_app/src/template_loader.rs`
- `TemplateLoaderRef` wrapper in `packages/rhd_chat/src/stream.rs` for use in chat operations
- Template files: `.md` extension, loaded by filename without extension
- Placeholder syntax: `{placeholderName}`
- Key templates:
  - `rhd_set_todo_list_contract.md`: Tool contract injected on first message
  - `todo_list_empty.md`: Prompt when no todo list exists
  - `todo_list_with_items.md`: Table format for todo items
  - `environment_details_with_role.md`: Environment details with active role
  - `environment_details_no_role.md`: Environment details without role

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
- **`attach_project(chat_id, project_name, event_sender)`**: Attaches project to chat, starts MCP clients, emits `ProjectAttached` event. Called synchronously from WebSocket handler (not spawned) to ensure errors propagate to WebSocket client.

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
- `TodoListUpdated { chat_id, items }`: Todo list was updated by AI tool call
- `RoleChanged { chat_id, project_name, role_name }`: Active role changed
- `RolesUpdated { chat_id }`: Roles list updated (project attached/detached)
- `ActiveRoleCleared { chat_id }`: Active role cleared

## Chat Database

- Chat data persisted in SQLite database at `<dbDir>/chats.db` (default: `rhd_db/chats.db`)
- Two tables: `chats` and `messages` with foreign key relationship
- `chats` table: `id` (INTEGER PRIMARY KEY), `title` (TEXT), `created_at` (TEXT), `updated_at` (TEXT), `active_model` (TEXT, nullable), `active_role_project` (TEXT, nullable), `active_role_name` (TEXT, nullable), `roles_list_injected` (BOOLEAN), `role_prompt_pending` (BOOLEAN), `todo_list` (TEXT, nullable)
- `messages` table: `id` (INTEGER PRIMARY KEY), `chat_id` (INTEGER FK), `role` (TEXT), `content` (TEXT), `created_at` (TEXT), `model` (TEXT, nullable), `thinking_content` (TEXT, nullable)
- Index on `messages.chat_id` for faster retrieval
- CASCADE DELETE: deleting a chat removes all its messages
- `add_message()` automatically updates chat's `updated_at` timestamp
- `truncate_messages(chat_id, after_message_id)`: deletes messages with id > after_message_id (for edit-and-resend)
- WAL mode and foreign keys enabled
- Thread-safe via `Mutex<Connection>`

### Todo List Storage
- Todo list stored as raw markdown string in `chats.todo_list` column
- `set_todo_list(chat_id, todo_list)`: Updates todo list for a chat
- `get_todo_list(chat_id)`: Retrieves todo list for a chat
- Parsed into `TodoItem` structs with `TodoStatus` enum (Pending, InProgress, Completed, Discarded)
- Checkbox syntax: `[ ]` (pending), `[-]` (in progress), `[x]` (completed), `[!]` (discarded)

### Model Tracking

- Each chat tracks its `active_model` (the currently selected model for that chat)
- Each message stores which `model` was used to generate it
- When sending a message, the chat's `active_model` is updated to match the model used
- Model indicators are shown in the UI when the model changes between messages (visual only, not sent to AI)

### Database Migration

- On initialization, `ChatDb` checks if columns exist and adds them if missing
- Migrated columns: `active_model`, `model`, `thinking_content`, `active_role_project`, `active_role_name`, `roles_list_injected`, `role_prompt_pending`, `todo_list`
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
