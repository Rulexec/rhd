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
    ├── plugin_states.rs # plugin_states table ops (upsert, tombstone remove, get)
    ├── schema.rs     # Database schema definitions
    └── tests/
        ├── mod.rs
        ├── helpers.rs
        ├── chat_tests.rs
        ├── message_tests.rs
        ├── migration_tests.rs
        └── plugin_state_tests.rs

packages/rhd_mcp_client/src/
├── lib.rs            # McpConfig, ToolDefinition, ToolResult, McpClientTrait
├── client.rs         # MCP client implementation
├── protocol.rs       # MCP JSON-RPC protocol types
├── transport.rs      # stdio transport for MCP servers
└── builtin.rs        # Built-in tools (rhd_set_flag)

packages/rhd_chat_api/src/
├── common.rs           # Chat, Message, PluginState, StateFormat, StateVersionRef types
├── methods/
│   ├── update_plugin_state.rs        # updatePluginState request/response
│   ├── remove_plugin_state.rs        # removePluginState request/response
│   ├── get_plugin_states.rs          # getPluginStates (filters: pluginId, schema)
│   ├── subscribe_plugin_states.rs    # subscribePluginStates (catch-up by held versions)
│   └── unsubscribe_plugin_states.rs  # unsubscribePluginStates
└── events/
    ├── plugin_state_changed.rs       # pluginStateChanged event payload
    └── plugin_state_removed.rs       # pluginStateRemoved event payload

packages/rhd_chat_server/src/
├── subscriptions.rs    # SubscriptionManager (incl. plugin_states_subscribers)
└── handlers/
    └── plugin_state.rs # Plugin state handlers (register-before-snapshot subscribe)

packages/rhd_chat_client/src/
└── client.rs           # ChatClient incl. typed plugin state methods + on_plugin_state_event

plugins/rhd_plugin_mcp/src/
└── status.rs           # McpStatusTracker: per-server run status, mcpStatus:1 state pushes

frontend/src/lib/components/
├── McpStatusList.svelte # MCPs tab rendering mcpStatus:1 states per server
└── PluginList.svelte    # Plugins tab with collapsed "State (n)" sections

```
