# Architecture

## Core Components

**rhd_ai_client**:
- AI client wrapper for OpenAI-compatible APIs
- Replaces the old `rhd_ai` package
- Provides streaming chat completions with tool support

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

**rhd_chat_api**:
- API types for chat WebSocket protocol
- Defines request/response types for chat operations
- Event types for real-time updates

**rhd_chat_server**:
- WebSocket server for chat storage and management
- Handles chat persistence, message streaming, and plugin management
- Uses `rhd_db` for data persistence

**rhd_chat_client**:
- WebSocket client for connecting to `rhd_chat_server`
- Provides methods for chat operations: list chats, get chat, create chat, add messages, etc.
- Handles real-time event subscriptions and streaming

**rhd_app** (CLI tool):
- Command-line interface for interacting with `rhd_chat_server`
- Uses `rhd_chat_client` for WebSocket communication
- Commands: `chats list`, `messages <chat_id>`, `queue <chat_id>`, `create-chat <title>`, `plugins list`, `plugins remove <plugin_id>`, `queue add <chat_id> <content>`
- Outputs JSON for easy parsing and scripting

**rhd_plugin_ai_completions**:
- AI completions plugin for the chat server
- Processes queued messages and generates AI responses
- Integrates with `rhd_chat_client` and `rhd_chat_server`

**rhd_mock_ai_provider**:
- Mock AI provider for testing
- Simulates AI API responses for integration testing
