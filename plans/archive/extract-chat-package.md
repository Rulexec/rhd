# Extract Chat Logic to rhd_chat Package

## Goal
Extract all chat-related logic from `packages/rhd_app/src/chat.rs` into a new `rhd_chat` package, splitting the monolithic file into smaller, focused modules.

## Current State
- `chat.rs` is ~970 lines
- Contains: CRUD, streaming, tool execution, project integration, pause/resume
- Dependencies: `rhd_db`, `rhd_ai`, `rhd_mcp_client`, `ProjectManager`, `McpServerCache`
- Coupling issue: `ProjectManager` and `McpServerCache` live in `rhd_app`

## Architecture

### New Package Structure
```
packages/rhd_chat/
├── Cargo.toml
└── src/
    ├── lib.rs           # Public exports
    ├── error.rs         # ChatError enum
    ├── event.rs         # ChatEvent enum
    ├── state.rs         # StreamState enum
    ├── manager.rs       # ChatManager struct + CRUD
    ├── stream.rs        # Streaming logic (send_message, edit_and_resend)
    ├── tools.rs         # Tool execution (tool_loop, collect_tools)
    └── projects.rs      # Project attachment logic
```

### Decoupling Strategy
Define traits in `rhd_chat` for external dependencies:

```rust
// Trait for project/MCP operations
pub trait ProjectProvider: Send + Sync {
    async fn get_mcp_status(&self, project_name: &str) -> Vec<(String, McpStatus)>;
    async fn get_mcp_clients(&self, project_name: &str) -> Vec<(String, Arc<McpClient>)>;
    fn get_project(&self, name: &str) -> Option<&Project>;
    async fn spawn_project_mcp(&self, project_name: &str) -> Result<(), String>;
}

// ChatManager becomes generic
pub struct ChatManager<P: ProjectProvider> {
    db: Arc<ChatDb>,
    active_streams: Mutex<HashMap<i64, StreamState>>,
    project_provider: Arc<P>,
}
```

### Module Responsibilities

| Module | Responsibility |
|--------|----------------|
| `error.rs` | `ChatError` enum with all error variants |
| `event.rs` | `ChatEvent` enum for streaming events |
| `state.rs` | `StreamState` (Running/Paused) with cancel/pause logic |
| `manager.rs` | `ChatManager` struct, CRUD operations, `new()`, `abort_chat()`, `pause_chat()`, `resume_chat()` |
| `stream.rs` | `send_message()`, `edit_and_resend()`, streaming callbacks |
| `tools.rs` | `tool_loop()`, `collect_tools_from_projects()`, `execute_tool_call()`, `split_tool_name()` |
| `projects.rs` | `attach_project()`, `detach_project()`, `get_chat_projects()`, system prompt injection |

## Implementation Steps

### Phase 1: Create rhd_chat package structure
1. Create `packages/rhd_chat/Cargo.toml` with dependencies
2. Create `packages/rhd_chat/src/lib.rs` with module declarations
3. Create empty module files

### Phase 2: Extract types and traits
4. Move `ChatError` to `error.rs`
5. Move `ChatEvent` to `event.rs`
6. Move `StreamState` to `state.rs`
7. Define `ProjectProvider` trait in `lib.rs`
8. Define `McpStatus` enum in `lib.rs` (or move from `project_manager.rs`)

### Phase 3: Extract ChatManager
9. Move `ChatManager` struct to `manager.rs` with generic `P: ProjectProvider`
10. Move CRUD methods (`create_chat`, `list_chats`, `get_chat`, `delete_chat`)
11. Move control methods (`abort_chat`, `pause_chat`, `resume_chat`)

### Phase 4: Extract streaming logic
12. Move `send_message()` to `stream.rs`
13. Move `edit_and_resend()` to `stream.rs`
14. Extract common streaming callback logic to helper function

### Phase 5: Extract tool execution
15. Move `tool_loop()` to `tools.rs`
16. Move `collect_tools_from_projects()` to `tools.rs`
17. Move `execute_tool_call()` to `tools.rs`
18. Move `split_tool_name()` to `tools.rs`

### Phase 6: Extract project integration
19. Move `attach_project()` to `projects.rs`
20. Move `detach_project()` to `projects.rs`
21. Move `get_chat_projects()` to `projects.rs`
22. Extract system prompt injection logic

### Phase 7: Update rhd_app
23. Implement `ProjectProvider` trait for `ProjectManager` in `rhd_app`
24. Update `DaemonState` to use `ChatManager<ProjectManager>`
25. Update imports in `daemon.rs`, `ws.rs`
26. Remove old `chat.rs` from `rhd_app`

### Phase 8: Update workspace
27. Add `rhd_chat` to workspace `Cargo.toml`
28. Update `rhd_app/Cargo.toml` to depend on `rhd_chat`

## Dependency Graph

```
rhd_chat
├── rhd_db (ChatDb)
├── rhd_ai (OpenAiClient, ChatMessage)
├── rhd_api (Project, ProjectInfo)
└── rhd_mcp_client (McpClient, McpClientTrait)

rhd_app
├── rhd_chat (ChatManager, ChatEvent)
├── rhd_db
├── rhd_ai
├── rhd_api
├── rhd_mcp_client
└── implements ProjectProvider for ProjectManager
```

## Benefits
- Clear separation of concerns
- Each module < 200 lines
- Testable in isolation
- Reusable chat logic
- Reduced coupling via traits
