# File Structure Reference

> **Note**: This file list may not be fully accurate. If you find issues like references to non-existing files or inaccurate descriptions, fix them immediately.

```
packages/rhd_app/src/
├── main.rs           # CLI entry point, command dispatch
├── cli.rs            # clap argument definitions
└── commands/
    ├── mod.rs        # Command module exports
    ├── chats.rs      # List chats command
    ├── messages.rs   # View chat messages command
    ├── queue.rs      # View queued messages command
    ├── create_chat.rs # Create chat command
    ├── plugins.rs    # List plugins command
    ├── remove_plugin.rs # Remove plugin command
    └── add_queue.rs  # Add queued message command

packages/rhd_db/src/
├── lib.rs            # Module exports, ScenarioDb, DbError
├── tests.rs          # DB tests
└── chat_db/
    ├── mod.rs        # ChatDb, ChatInfo, Message, chat/message persistence
    ├── chats.rs      # Chat CRUD operations
    ├── messages.rs   # Message CRUD operations
    ├── schema.rs     # Database schema definitions
    └── tests/
        ├── mod.rs
        ├── helpers.rs
        ├── chat_tests.rs
        ├── message_tests.rs
        └── migration_tests.rs

packages/rhd_mcp_client/src/
├── lib.rs            # McpConfig, ToolDefinition, ToolResult, McpClientTrait
├── client.rs         # MCP client implementation
├── protocol.rs       # MCP JSON-RPC protocol types
├── transport.rs      # stdio transport for MCP servers
└── builtin.rs        # Built-in tools (rhd_set_flag)

```
