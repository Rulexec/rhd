# File Structure Reference

> **Note**: This file list may not be fully accurate. If you find issues like references to non-existing files or inaccurate descriptions, fix them immediately.

```
frontend/
├── .nvmrc              # Node.js version (v24.13.0)
├── package.json        # Dependencies and scripts
├── tsconfig.json       # TypeScript configuration
├── vite.config.js      # Vite configuration
├── vitest.config.unit.ts  # Unit test config
├── vitest.config.e2e.ts   # E2E test config
├── index.html          # Entry HTML
├── svelte.config.js    # Svelte configuration
└── src/
    ├── main.ts         # App entry point
    ├── vite-env.d.ts   # Vite/Svelte type declarations
    ├── App.svelte      # Root component
    ├── lib/
    │   ├── ws.ts       # WebSocket connection service with Zod validation
    │   ├── stores.ts   # Svelte stores for scenario state
    │   ├── chatStores.ts # Svelte stores for chat state
    │   ├── chatWs.ts   # Chat WebSocket functions and event handlers
    │   ├── utils.ts    # Helper functions
    │   ├── router.ts   # Hash-based routing
    │   └── types/
    │       ├── index.ts    # Domain types with Zod schemas
    │       └── ws.ts       # WebSocket protocol schemas
    ├── components/
    │   ├── TabNav.svelte
    │   ├── ScenariosTab.svelte
    │   ├── ChatsTab.svelte
    │   ├── ChatList.svelte
    │   ├── ChatView.svelte
    │   ├── MessageList.svelte
    │   ├── Message.svelte
    │   ├── MessageInput.svelte
    │   ├── StreamingMessage.svelte
    │   ├── ActiveScenario.svelte
    │   └── FinishedScenario.svelte
    ├── styles/
    │   ├── global.css
    │   ├── utilities.css
    │   └── components/
    └── tests/
        ├── setup.ts        # Test setup
        ├── testUtils.ts    # E2E test utilities
        └── e2e/
            ├── chat.test.ts
            ├── chat-messageflow.test.ts
            └── chat-streaming.test.ts

packages/rhd_app/src/
├── main.rs           # CLI entry point, command dispatch
├── cli.rs            # clap argument definitions
├── config.rs         # DaemonConfig YAML loading
├── daemon.rs         # Unix socket server, WebSocket server, connection handling
├── client.rs         # Unix socket client
├── execution.rs      # ExecutionTracker, ExecutionHandle, execution tracking
├── chat.rs           # ChatManager, ChatEvent, ChatError
├── ws.rs             # WebSocket server, JSON protocol handlers, chat handlers
├── log.rs            # LogSink, execution logging, meta.json writing/reading
├── mcp_cache.rs      # MCP server instance caching
├── mcp_loader.rs     # MCP config loading from mcp/<name>/mcp.yaml
├── ipc/
│   ├── mod.rs
│   └── protocol.rs   # rkyv message types, read/write helpers
└── scenario/
    ├── mod.rs        # Action/Scenario structs
    ├── loader.rs     # YAML loading, validation
    ├── executor.rs   # Action execution engine
    ├── error.rs      # ExecuteError, ExecuteOutput types
    ├── ai_chat.rs    # AI chat execution with token tracking
    ├── run_command.rs # Command execution with section tracking
    └── placeholder.rs # Placeholder resolution, ExecutionContext

packages/rhd_ai/src/
├── lib.rs
├── config.rs         # ModelConfig with optional token pricing, load_models()
└── client.rs         # OpenAiClient, ChatMessage, streaming support, AiError

packages/rhd_api/src/
└── lib.rs            # Shared types: ExecutionEvent, StepTiming, LogSection, TokenUsage, ScenarioMeta, WsRequest, WsResponse, WsEvent, ErrorCode, TokenPriceTier, Chat event types

packages/rhd_util/src/
└── lib.rs            # RhdError, RhdResult, substitute_env_vars()

packages/rhd_db/src/
├── lib.rs            # Module exports, ScenarioDb, DbError, SQLite wrapper for ID persistence
└── chat_db.rs        # ChatDb, ChatInfo, Message, chat/message persistence

packages/rhd_mcp_client/src/
├── lib.rs            # McpConfig, ToolDefinition, ToolResult, McpClientTrait
├── client.rs         # MCP client implementation
├── protocol.rs       # MCP JSON-RPC protocol types
├── transport.rs      # stdio transport for MCP servers
└── builtin.rs        # Built-in tools (rhd_set_flag)

packages/rhd_test/src/
├── main.rs           # E2E test runner entry point
├── args.rs           # CLI argument definitions
├── mock_server.rs    # Mock OpenAI-compatible server with token usage and streaming
├── control_server.rs # HTTP control server for test coordination
├── standard_test.rs  # Standard test with meta.json validation
├── mcp_test.rs       # MCP tool test
├── sse_test.rs       # SSE streaming test
├── frontend_test.rs  # Frontend E2E test orchestrator
└── utils.rs          # Test utilities
```
