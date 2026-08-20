# File Structure Reference

> **Note**: This file list may not be fully accurate. If you find issues like references to non-existing files or inaccurate descriptions, fix them immediately.

```
packages/rhd_app/src/
├── main.rs           # CLI entry point, command dispatch
├── cli.rs            # clap argument definitions
├── config.rs         # DaemonConfig YAML loading
├── client.rs         # Unix socket client
├── log.rs            # LogSink, execution logging, meta.json writing/reading
├── mcp_cache.rs      # MCP server instance caching
├── mcp_loader.rs     # MCP config loading from mcp/<name>/mcp.yaml
├── project_loader.rs # Project loading from projects/<name>/
├── project_manager.rs # Project management, MCP client lifecycle
├── notifications.rs  # Desktop notifications (terminal-notifier, osascript)
├── credentials.rs    # Credentials file loading
├── template_loader.rs # Template loading from templates/ directory
├── templates.rs      # Template types and rendering
├── lib.rs            # Module exports
├── daemon/
│   ├── mod.rs        # Daemon module, DaemonState, server setup
│   ├── ipc.rs        # Unix socket server, IPC request handling
│   ├── reload.rs     # Reload logic, config reloading
│   └── run.rs        # Scenario execution orchestration
├── execution/
│   ├── mod.rs        # Execution module exports
│   ├── tracker.rs    # ExecutionTracker, step timing, token tracking
│   └── handle.rs     # ExecutionHandle, FinishedExecution
├── ipc/
│   ├── mod.rs
│   └── protocol.rs   # rkyv message types, read/write helpers
├── scenario/
│   ├── mod.rs        # Action/Scenario structs
│   ├── loader.rs     # YAML loading, validation
│   ├── executor.rs   # Action execution engine
│   ├── error.rs      # ExecuteError, ExecuteOutput types
│   ├── run_command.rs # Command execution with section tracking
│   ├── placeholder.rs # Placeholder resolution, ExecutionContext
│   └── ai_chat/
│       ├── mod.rs    # AI chat module exports
│       ├── simple.rs # Single-shot AI chat (no tools)
│       ├── mcp.rs    # AI chat with MCP tool loop
│       └── utils.rs  # Shared AI chat utilities
├── project_loader/
│   └── tests.rs      # Project loader tests
├── project_manager/
│   └── tests.rs      # Project manager tests
├── template_loader/
│   └── tests.rs      # Template loader tests
├── scenario/placeholder/
│   └── tests.rs      # Placeholder resolution tests
└── ws/
    ├── mod.rs        # WebSocket server setup
    ├── events.rs     # WebSocket event types
    ├── tests.rs      # WebSocket tests
    └── handlers/
        ├── mod.rs    # Handler module exports
        ├── chat.rs   # Chat WebSocket handlers
        ├── project.rs # Project WebSocket handlers
        ├── role.rs   # Role WebSocket handlers
        └── scenario.rs # Scenario WebSocket handlers

packages/rhd_chat/src/
├── lib.rs            # Module exports, ProjectProvider trait
├── manager.rs        # ChatManager, chat operations
├── event.rs          # ChatEvent enum
├── error.rs          # ChatError types
├── state.rs          # Chat state management
├── projects.rs       # Project attachment/detachment
├── chat_log.rs       # Chat logging (ChatLogSink, RawChatLogSink)
├── stream/
│   ├── mod.rs        # Stream module exports
│   ├── contract.rs   # Tool contract injection
│   ├── send/
│   │   ├── mod.rs    # Send module exports
│   │   ├── message.rs # Message sending logic
│   │   ├── tools.rs  # Tool call handling
│   │   └── utils.rs  # Stream utilities
│   └── tests.rs      # Stream tests
├── tools/
│   ├── mod.rs              # Tools module exports
│   ├── builtin.rs          # Built-in tools (rhd_set_flag, rhd_set_todo_list, rhd_set_role)
│   ├── builtin_fsms.rs     # BuiltinFsmManager coordinating helper FSMs
│   ├── db_sync_listener.rs # DB synchronization listener for FSM events
│   ├── fsm_wrapper.rs      # FsmToolLoop async wrapper driving ToolLoopFsm
│   ├── messages.rs         # Message building for tool loop
│   ├── tool_loop.rs        # Tool loop entry point (delegates to FsmToolLoop)
│   ├── utils.rs            # Tool utilities
│   └── tests/
│       ├── mod.rs
│       ├── helpers.rs
│       ├── mock_mcp.rs
│       ├── message_tests.rs
│       ├── role_tests.rs
│       ├── todo_list.rs
│       └── fsm_integration_tests.rs  # FSM and DB sync listener integration tests
└── projects/
    └── tests.rs      # Project tests

packages/rhd_ai/src/
├── lib.rs
├── config.rs         # ModelConfig with optional token pricing, load_models()
├── config/
│   └── tests.rs      # Config tests
└── client/
    ├── mod.rs        # OpenAiClient, ChatMessage, streaming support, AiError
    ├── chat.rs       # Chat completion methods
    ├── types.rs      # Client types (ToolCall, FunctionCall, etc.)
    └── stream/
        ├── mod.rs    # Stream module exports
        ├── simple.rs # Simple streaming (no tools)
        └── tools.rs  # Streaming with tool definitions

packages/rhd_api/src/
├── lib.rs            # Module exports
├── chat.rs           # Chat request/response types, ChatMessageDto, ChatEvent types
├── execution.rs      # ExecutionEvent, StepTiming, LogSection, TokenUsage, ScenarioMeta
├── project.rs        # Project-related types
├── ws.rs             # WsRequest, WsResponse, WsEvent, ErrorCode
└── tests.rs          # API tests

packages/rhd_util/src/
├── lib.rs            # RhdError, RhdResult, substitute_env_vars()
└── tests.rs          # Util tests

packages/rhd_db/src/
├── lib.rs            # Module exports, ScenarioDb, DbError
├── tests.rs          # DB tests
└── chat_db/
    ├── mod.rs        # ChatDb, ChatInfo, Message, chat/message persistence
    ├── chats.rs      # Chat CRUD operations
    ├── messages.rs   # Message CRUD operations
    ├── projects.rs   # Project-related DB operations
    ├── schema.rs     # Database schema definitions
    └── tests/
        ├── mod.rs
        ├── helpers.rs
        ├── chat_tests.rs
        ├── message_tests.rs
        ├── migration_tests.rs
        ├── project_tests.rs
        ├── role_tests.rs
        └── todo_tests.rs

packages/rhd_mcp_client/src/
├── lib.rs            # McpConfig, ToolDefinition, ToolResult, McpClientTrait
├── client.rs         # MCP client implementation
├── protocol.rs       # MCP JSON-RPC protocol types
├── transport.rs      # stdio transport for MCP servers
└── builtin.rs        # Built-in tools (rhd_set_flag)

packages/rhd_test/src/
├── main.rs           # E2E test runner entry point
├── args.rs           # CLI argument definitions
├── control_server.rs # HTTP control server for test coordination
├── daemon_startup_test.rs # Daemon startup validation
├── mcp_test.rs       # MCP tool test
├── mock_mcp_server.rs # Mock MCP server for testing
├── sse_test.rs       # SSE streaming test
├── utils.rs          # Test utilities
├── mock_server/
│   ├── mod.rs        # Mock server module
│   ├── handlers.rs   # Mock AI server handlers
│   └── types.rs      # Mock server types
└── standard_test/
    ├── mod.rs        # Standard test module
    ├── execution.rs  # Test execution logic
    ├── setup.rs      # Test setup
    └── validation.rs # Test validation
```
