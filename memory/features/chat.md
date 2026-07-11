# Chat

## Purpose
Persistent conversational interface for direct AI interaction. Users create chats, send messages, receive streaming responses, and can edit/resend previous messages. All conversations persist across daemon restarts.

## Delete All Chats
- "Delete all chats" button at the bottom of the chat list sidebar
- Removes all chats and their messages at once
- Shows confirmation dialog before deleting
- Disabled when no chats exist
- Clears all related stores (currentChatId, messages, chatProjects, mcpStatuses)

## How It Works

### Chat Lifecycle
1. User creates a new chat with a title
2. User selects a model from available models list
3. User sends messages; AI responds with streaming text
4. Messages persist in SQLite database (`rhd_db/chats.db`)
5. User can edit previous messages — this truncates the conversation after that point and resends to AI
6. User can abort streaming responses mid-generation
7. Reopening a chat restores the same model selection

### Model Selection
- Model selector dropdown appears under message input
- Available models fetched from daemon (real models only, not aliases)
- Selected model persists per chat — reopening chat shows same model
- Visual indicators in chat history show when model changes between messages
- Model indicators are visual-only, not sent to AI context

### Streaming Responses
- AI responses stream token-by-token via Server-Sent Events (SSE)
- Frontend shows animated dots indicator while streaming
- First chunk hides loader and shows assistant message with content
- Subsequent chunks append to content in real-time
- Stream finish removes animated dots, shows final message
- Empty chunks filtered on backend (not sent to frontend)

### Smart Auto-Scrolling
- Message list auto-scrolls to bottom when new content arrives **only if user is at bottom**
- Tracks "at bottom" state with 30px threshold from bottom
- If user scrolls up, auto-scroll stops (respects user's scroll position)
- If user scrolls back to bottom, auto-scroll resumes
- New streaming session resets auto-scroll to enabled
- Thinking content collapsible also has smart auto-scroll (see Thinking/Reasoning Content section)

### Thinking/Reasoning Content
- AI models with reasoning support (e.g., Qwen) emit `chatThinkingChunk` events
- Thinking content accumulated separately in `streamingThinkingContent` store
- Displayed in collapsible "Thinking" section (collapsed by default)
- **Collapsed preview**: Shows last 3 visual lines (rendered/wrapped) so user sees live streaming progress
- **Expanded state**: Auto-scrolls to bottom; continues auto-scroll while user stays at bottom; stops if user scrolls up; resumes if user scrolls back to bottom
- Persisted in `thinking_content` column of messages table
- Visible in message history for assistant messages
- Cleared on stream finish, stream error, and chat switch (avoid stale content)

### System Prompts
- System prompts emitted as `chatMessageAdded` events with `role="system"`
- Displayed in collapsible "System Prompt" section (collapsed by default)
- Sent before user message in `send_message` and `edit_and_resend`

### MCP Tool Calls
- Tool calls emit `chatToolCallStarted` and `chatToolCallCompleted` events
- Displayed in `ToolCallMessage` component with MCP ID
- Collapsed by default, showing header with status icon, MCP ID, tool name
- Expanded view shows arguments and result sections
- Tool calls attached to streaming assistant message via `toolCalls` array
- Tool names are namespaced: `{mcp_id}/{tool_name}` format (e.g., `fs1/read_file`)
- Built-in tools (e.g., `rhd_set_flag`) are not namespaced
- Tool call IDs are made globally unique using atomic counter in backend
- Event ordering: `ToolCallStarted` sent BEFORE `MessageAdded` (intermediate assistant) to ensure frontend creates temp message first

### Message Editing
- User can edit their own messages
- Editing truncates all messages after the edited message
- AI regenerates response from the edited point forward
- Model used for regeneration is the chat's active model

### Abort
- User can abort streaming response mid-generation
- Cancellation token stops the AI request
- Partial content may be displayed
- Error state shown with retry option

### Error Handling
- Network errors, API errors, and parse errors shown in UI
- Retry button resends the last message
- Error messages include context (model name, status code)

## Data Model

### Chats Table
- `id` — auto-increment primary key
- `title` — chat name
- `active_model` — currently selected model (nullable)
- `created_at`, `updated_at` — ISO 8601 UTC timestamps

### Messages Table
- `id` — auto-increment primary key
- `chat_id` — foreign key to chats (cascade delete)
- `role` — "user", "assistant", "system", or "tool"
- `content` — message text (JSON for assistant messages with tool calls, JSON for tool results)
- `model` — model used for this message (nullable, for visual indicators)
- `thinking_content` — reasoning/thinking content (nullable)
- `created_at` — ISO 8601 UTC timestamp

## WebSocket Protocol

### Requests
- `createChat` — create new chat with title
- `listChats` — get all chats
- `getChat` — get chat with messages
- `deleteChat` — delete chat and all messages
- `deleteAllChats` — delete all chats and their messages
- `sendMessage` — send user message, trigger AI response
- `editMessage` — edit message, truncate, resend
- `abortChat` — cancel active streaming
- `getAvailableModels` — list real models (no aliases)

### Events
- `chatStreamChunk` — partial content from AI
- `chatThinkingChunk` — partial thinking/reasoning content from AI
- `chatStreamFinished` — AI response complete
- `chatStreamError` — AI request failed
- `chatMessageAdded` — new message persisted (user, assistant, system, or tool)
- `chatUpdated` — chat metadata changed
- `chatToolCallStarted` — MCP tool call started (creates temp assistant message if none exists)
- `chatToolCallCompleted` — MCP tool call completed (updates tool call with result)
- `chatPaused` — chat paused during tool loop
- `chatResumed` — chat resumed from pause
- `projectAttached` — project attached to chat
- `projectDetached` — project detached from chat
- `projectMcpStatusChanged` — MCP status changed for attached project

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

## Key Files
- Chat database: `packages/rhd_db/src/chat_db.rs`
- Chat manager: `packages/rhd_chat/src/manager.rs`
- Tool loop: `packages/rhd_chat/src/tools.rs`
- Chat events: `packages/rhd_chat/src/event.rs`
- AI streaming client: `packages/rhd_ai/src/client.rs`
- WebSocket handlers: `packages/rhd_app/src/ws.rs`
- Frontend stores: `frontend/src/lib/chatStores.ts`
- Frontend WebSocket: `frontend/src/lib/chatWs.ts`
- Chat components: `frontend/src/components/Chat*.svelte`, `Message*.svelte`, `StreamingMessage.svelte`, `ToolCallMessage.svelte`
- Chat logging: `packages/rhd_chat/src/chat_log.rs`
