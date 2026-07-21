# Architecture

## Core Components

**rhd_util**: Shared error types (`RhdError`, `RhdResult<T>`), `substitute_env_vars()` for `$VAR` expansion in config strings

**rhd_ai**:
- `ModelConfig`: AI model configuration (model_id, baseUrl, apiKey, model, optional token pricing)
- `OpenAiClient`: HTTP client for chat completions API with streaming support
- `ChatMessage`: Public enum for building message history (System, User, Assistant, Tool variants)
- `chat_stream()`: Streaming chat completion with callback-based chunk processing
- `chat_stream_cancellable()`: Streaming with `CancellationToken` for abort support
- `chat_stream_with_tools()`: Streaming with tool definitions for MCP tool loop
- `StreamChunk`, `StreamResult`: Types for streaming response handling
- Loads models from `models/*.yaml` at startup
- Parses token usage from API responses

**rhd_api**:
- Shared types for execution tracking, WebSocket protocol, and token pricing
- `ExecutionEvent`, `StepTiming`, `LogSection`, `TokenUsage`, `ScenarioMeta`
- `WsRequest`, `WsResponse`, `WsEvent`, `ErrorCode` for WebSocket protocol
- Chat request types: `CreateChat`, `ListChats`, `GetChat`, `DeleteChat`, `SendMessage`, `EditMessage`, `AbortChat`
- Chat event types: `ChatStreamChunkEvent`, `ChatStreamFinishedEvent`, `ChatStreamErrorEvent`, `ChatMessageAddedEvent`, `ChatUpdatedEvent`
- `ChatMessageDto`: Data transfer object for chat messages
- `TokenPriceTier` and `calculate_cost()` for token pricing
- Error codes: `ChatNotFound`, `MessageNotFound`, `ChatStreamFailed`

**rhd_db**:
- `ScenarioDb`: SQLite database wrapper for persisting scenario execution IDs
- `ChatDb`: SQLite database wrapper for chat persistence (chats and messages)
- Uses WAL mode for better concurrency and crash recovery
- Thread-safe via `Mutex<Connection>`
- `next_id()`: Atomically retrieves and increments the next scenario ID
- Chat operations: `create_chat()`, `list_chats()`, `get_chat()`, `delete_chat()`, `update_chat_title()`, `touch_chat()`
- Message operations: `add_message()`, `get_messages()`, `get_message()`, `truncate_messages()`, `update_message()`
- Database files: `<dbDir>/meta.db` (scenarios), `<dbDir>/chats.db` (chats)

**rhd_mcp_client**:
- `McpConfig`: MCP server configuration (cmd, args, cwd, env)
- `McpClient`: MCP client implementation with stdio transport
- `ToolDefinition`, `ToolResult`: Tool types for MCP protocol
- `McpClientTrait`: Trait for MCP client implementations
- Built-in tools support (e.g., `rhd_set_flag`)
- `pid()`: Returns PID of spawned MCP server process (for logging during reload)

**rhd_chat**:
- `ChatManager`: Handles all chat operations with streaming AI responses
- `ChatEvent`: Enum for chat events (StreamChunk, ThinkingChunk, StreamFinished, StreamError, MessageAdded, ToolCallStarted, ToolCallCompleted, ChatPaused, ChatResumed, etc.)
- `tool_loop()`: Implements the MCP tool call loop with streaming support
- `collect_tools_from_projects()`: Gathers tools from attached projects' MCP clients
- `ChatLoggers`: Logging infrastructure for chat interactions
- `ProjectProvider`: Trait for accessing project MCP clients
- Tool call IDs are made globally unique using atomic counter
- Event ordering: ToolCallStarted sent BEFORE MessageAdded (intermediate assistant) to ensure frontend creates temp message first

**rhd_app**:
- **Daemon mode**: Unix socket server on `$HOME/rhd.sock` (default), accepts `RunScenario` and `Reload` requests
- **WebSocket server**: Optional TCP listener on `127.0.0.1:{ws_port}` for Web UI integration
- **Client mode**: Connects to daemon, sends scenario name, receives output. Also supports `rhd reload` command
- **Scenario executor**: Runs action chains sequentially with placeholder resolution
- **Execution tracking**: Tracks step timings, token usage, log sections
- **IPC protocol**: rkyv serialization with version-prefixed framing (Unix socket)
- **WebSocket protocol**: JSON over WebSocket (TCP)
- **MCP integration**: Loads MCP configs, caches server instances, handles tool calls
- **Daemon state structure**: `DaemonState` contains `inner: RwLock<ReloadableInner>` for reloadable config (scenarios, models, mcp_configs, default_model, project_manager, config_paths) and `reload_lock: RwLock<()>` for coordinating reload with active executions/chats
- **Reload mechanism**: `handle_reload()` acquires write lock on `reload_lock`, waits for active executions/chats to finish, reloads configs from disk, diffs MCP configs, stops removed MCP servers (with PID logging), restarts changed MCP servers, updates state
- **MCP cache extensions**: `stop_specific()` and `restart_specific()` methods for managing specific MCP servers by cache key
- **Role system**: Projects can define roles in `roles/<roleName>/` directories with `systemPrompt.md` and `whenToUse.md`. Roles are loaded by `project_loader.rs` and exposed via `ProjectProvider` trait. `ChatManager` provides `set_active_role()`, `clear_active_role()`, `get_active_role()`, and `get_available_roles()` methods. Role state tracked in `chats` table (`active_role_project`, `active_role_name`, `roles_list_injected`, `role_prompt_pending` columns).
- **Todo list system**: AI can manage task tracking via `rhd_set_todo_list` tool. Todo list stored as markdown in `chats.todo_list` column. Parsed into `TodoItem` structs with `TodoStatus` enum (Pending, InProgress, Completed, Discarded). Injected into AI context after each tool loop iteration via `inject_todo_list_message()`. Tool contract injected on first message via `inject_todo_tool_contract()`.
- **Template system**: Templates loaded at compile-time via `include_str!` macro in `templates.rs`. `TemplateLoader` provides `get_template()` and `render_template()` methods. Templates organized in `templates/mcp_internal/`, `templates/environment/`, and `templates/roles/` directories.
