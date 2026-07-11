# Phase 2: Project Manager & MCP Lifecycle

## Goal
Manage project MCP servers, track status, provide API.

## Current State Analysis
- [`McpServerCache`](packages/rhd_app/src/mcp_cache.rs:9) provides `get_or_spawn()` for MCP client caching
- [`DaemonState`](packages/rhd_app/src/daemon.rs) holds shared state including `mcp_cache`
- WebSocket protocol in [`packages/rhd_api/src/lib.rs`](packages/rhd_api/src/lib.rs:150) uses tagged enum `WsRequest`
- Chat events pattern in [`ChatEvent`](packages/rhd_app/src/chat.rs:30) - broadcast channel

## Subtasks

### 2.1. Create ProjectManager
**File**: `packages/rhd_app/src/project_manager.rs` (new)

**State**:
```rust
pub struct ProjectManager {
    projects: HashMap<String, Project>,
    mcp_clients: HashMap<String, Arc<McpClient>>,  // key: "project_name:mcp_name"
    mcp_status: HashMap<String, McpStatus>,         // key: "project_name:mcp_name"
}

pub enum McpStatus {
    Connecting,
    Connected,
    Failed(String),  // error message
}
```

**Methods**:
- `new(projects: Vec<Project>) -> Self`
- `list_projects() -> Vec<ProjectInfo>` - returns project metadata without internals
- `get_project(name: &str) -> Option<&Project>`
- `get_mcp_status(project_name: &str) -> Vec<(String, McpStatus)>` - status of all MCPs for project
- `spawn_project_mcp(project_name: &str, mcp_cache: &McpServerCache) -> Result<()>` - spawn all MCP servers for project
- `get_mcp_clients(project_name: &str) -> Vec<Arc<McpClient>>` - get connected clients

**Design decisions**:
- ProjectManager does NOT own McpServerCache - receives reference when spawning
- Status tracked separately from clients to handle failed spawns
- Key format: `"{project_name}:{mcp_name}"` for unique identification

### 2.2. MCP server spawning
**Reuse**: [`McpServerCache`](packages/rhd_app/src/mcp_cache.rs:9) for client caching

**Logic**:
- When `spawn_project_mcp()` called:
  - For each `McpRef` in project's `mcp_configs`:
    - Build `McpConfig` from ref (resolve args/env overrides)
    - Set status to `Connecting`
    - Call `mcp_cache.get_or_spawn()`
    - On success: store client, set status `Connected`
    - On failure: set status `Failed(error)`
- Servers stay alive until daemon shutdown (not on detach)

**Error handling**:
- Individual MCP failure doesn't block other MCPs
- Failed status includes error message for UI display

### 2.3. Add WebSocket API for projects
**File**: [`packages/rhd_api/src/lib.rs`](packages/rhd_api/src/lib.rs:150)

**New WsRequest variants**:
```rust
#[serde(rename = "listProjects")]
ListProjects { id: String },

#[serde(rename = "getProjectMcpStatus", rename_all = "camelCase")]
GetProjectMcpStatus { id: String, project_name: String },
```

**New WsEvent types**:
```rust
ProjectMcpStatusChanged {
    project_name: String,
    mcp_name: String,
    status: McpStatusDto,  // serializable version
}
```

**File**: [`packages/rhd_app/src/ws.rs`](packages/rhd_app/src/ws.rs)

**Handler logic**:
- `ListProjects` → call `project_manager.list_projects()`, return as JSON array
- `GetProjectMcpStatus` → call `project_manager.get_mcp_status()`, return status array
- Emit `ProjectMcpStatusChanged` events when status changes during spawn

### 2.4. Integrate with DaemonState
**File**: [`packages/rhd_app/src/daemon.rs`](packages/rhd_app/src/daemon.rs)

**Changes**:
- Add `project_manager: Arc<ProjectManager>` to `DaemonState`
- Initialize on daemon startup:
  - Call `load_projects(config.projects_dir)`
  - Create `ProjectManager::new(projects)`
  - Wrap in `Arc`
- Pass to WebSocket handler for request handling

**Startup behavior**:
- Projects loaded once at daemon start
- MCP servers NOT spawned until project attached to chat (lazy loading)

## Deliverables
- [ ] `ProjectManager` with MCP lifecycle management
- [ ] MCP status tracking (connecting/connected/failed)
- [ ] WebSocket API: `listProjects`, `getProjectMcpStatus`
- [ ] WebSocket event: `projectMcpStatusChanged`
- [ ] Integration with [`DaemonState`](packages/rhd_app/src/daemon.rs)
- [ ] Unit tests for ProjectManager

## Dependencies
- Phase 1 (Project data model) must be complete

## Risk Assessment
- **Medium risk**: MCP server spawning may fail, need robust error handling
- **Unknown**: How to handle MCP server crashes after initial spawn
- **Mitigation**: Status tracking allows UI to show errors, retry logic can be added later
