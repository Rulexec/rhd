# RHD Reload Command Plan

## Overview

Add `rhd reload` CLI command that communicates with the daemon via `rhd.sock` to reload YAML configs (scenarios, projects, MCP, models) without restarting the daemon. The reload should:
- Wait for running scenarios and AI chat streams to finish
- Block new scenarios/chats silently until reload completes (no errors)
- Restart only MCP servers whose configs changed
- Stop MCP servers removed from config

## Architecture

```
┌─────────────────┐     IPC (rhd.sock)     ┌─────────────────────────────────┐
│  rhd reload     │ ──────────────────────► │  Daemon                         │
│  (client)       │  IpcRequest::Reload    │                                 │
└─────────────────┘                         │  1. Acquire reload lock         │
                                            │  2. Wait for active executions  │
                                            │  3. Wait for active chats       │
                                            │  4. Reload configs              │
                                            │  5. Diff MCP configs            │
                                            │  6. Stop removed MCPs (log PID) │
                                            │  7. Restart changed MCPs        │
                                            │  8. Update state                │
                                            │  9. Release lock                │
                                            └─────────────────────────────────┘
```

## Implementation Steps

### 1. IPC Protocol Extension

**File**: `packages/rhd_app/src/ipc/protocol.rs`

Add `Reload` variant to `IpcRequest`:
```rust
pub enum IpcRequest {
    RunScenario { ... },
    DaemonNotification,
    FrontendNotification,
    TestScenarioStarted { ... },
    TestScenarioFinished { ... },
    Reload,  // NEW
}
```

Add `Reloaded` variant to `IpcResponse`:
```rust
pub enum IpcResponse {
    Success { ... },
    Error { ... },
    Aborted,
    Paused { ... },
    Ok,
    Reloaded {  // NEW
        scenarios_reloaded: usize,
        models_reloaded: usize,
        mcp_restarted: usize,
        mcp_stopped: usize,
        projects_reloaded: usize,
    },
}
```

### 2. CLI Extension

**File**: `packages/rhd_app/src/cli.rs`

Add `Reload` subcommand:
```rust
pub enum Command {
    Daemon(DaemonArgs),
    Run(RunArgs),
    Dev(DevArgs),
    Reload(ReloadArgs),  // NEW
}

pub struct ReloadArgs {
    #[arg(long)]
    pub socket: Option<PathBuf>,
}
```

### 3. Client Function

**File**: `packages/rhd_app/src/client.rs`

Add `send_reload()` function similar to `send_daemon_notification()`:
- Connect to socket
- Send `IpcRequest::Reload`
- Receive `IpcResponse::Reloaded` with stats
- Print summary to stdout

### 4. Main Command Handler

**File**: `packages/rhd_app/src/main.rs`

Add handler for `Command::Reload`:
```rust
Command::Reload(args) => {
    let socket_path = args.socket.unwrap_or_else(cli::default_socket_path);
    if let Err(err) = client::send_reload(&socket_path).await {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
```

### 5. Daemon State Refactoring — Inner Grouping

**File**: `packages/rhd_app/src/daemon.rs`

Group all reloadable config under a single `Inner` struct with one `RwLock` to avoid lock chains and deadlocks:

```rust
pub struct ReloadableInner {
    pub scenarios: HashMap<String, Scenario>,
    pub models: HashMap<String, ModelConfig>,
    pub mcp_configs: HashMap<String, McpConfig>,
    pub default_model: Option<String>,
    pub project_manager: Arc<ProjectManager>,
    pub config_paths: ResolvedConfigPaths,
}

pub struct DaemonState {
    // Reloadable config — single lock
    pub inner: RwLock<ReloadableInner>,
    
    // Non-reloadable / separate concerns
    pub mcp_cache: Arc<McpServerCache>,
    pub logs: Option<std::path::PathBuf>,
    pub execution_tracker: Arc<ExecutionTracker>,
    pub chat_db: Arc<ChatDb>,
    pub chat_manager: Arc<ChatManager>,
    pub chat_event_sender: broadcast::Sender<ChatEvent>,
    pub frontend_alive: Arc<AtomicBool>,
    pub never_fail: bool,
    pub ws_port: Option<u16>,
    
    // Reload coordination
    pub reload_lock: RwLock<()>,  // Write-held during reload, read-held during executions
}

pub struct ResolvedConfigPaths {
    pub config_file: PathBuf,
    pub models_dir: PathBuf,
    pub scenarios_dir: PathBuf,
    pub mcp_dir: PathBuf,
    pub projects_dir: PathBuf,
    pub credentials_config: Option<PathBuf>,
}
```

### 6. Execution Gating — Silent Wait

**File**: `packages/rhd_app/src/daemon.rs`

Modify `handle_run_scenario()` to **wait** (not error) when reload is in progress:
```rust
fn handle_run_scenario(...) -> Vec<IpcResponse> {
    // Acquire read lock — blocks silently if reload holds write lock
    let inner_guard = state.inner.read();  // Blocks during reload, no error
    
    // Also hold reload_lock read to prevent reload from starting while we're setting up
    let _reload_guard = state.reload_lock.read();
    
    // ... rest of handler uses inner_guard.scenarios, inner_guard.models, etc.
}
```

This ensures:
- New scenarios wait silently if reload is in progress (no error returned)
- Reload waits for already-started scenarios to finish (they hold `reload_lock` read)

### 7. Chat Manager Gating — Silent Wait

**File**: `packages/rhd_app/src/chat.rs`

Pass `reload_lock` reference to `ChatManager` or use shared flag. When user sends message during reload:
- `send_message()` should **wait** for reload to finish, not return error
- Same for `edit_and_resend()`

```rust
// In send_message():
let _reload_guard = state.reload_lock.read();  // Blocks silently during reload
// ... proceed with message handling
```

### 8. Reload Handler Implementation

**File**: `packages/rhd_app/src/daemon.rs`

Add `handle_reload()` function:
```rust
fn handle_reload(state: &DaemonState) -> Vec<IpcResponse> {
    // 1. Acquire write lock on reload_lock (blocks until all executions/chats release read locks)
    let _reload_guard = state.reload_lock.write();
    
    // 2. Reload configs (outside inner lock to minimize lock time)
    let config_paths = {
        let inner = state.inner.read();
        inner.config_paths.clone()
    };
    
    let credentials = if let Some(cred_path) = &config_paths.credentials_config {
        if cred_path.exists() {
            match credentials::load_credentials(cred_path) {
                Ok(c) => c,
                Err(e) => return vec![IpcResponse::Error { message: e }],
            }
        } else {
            return vec![IpcResponse::Error { 
                message: format!("credentials file not found: {}", cred_path.display()) 
            }];
        }
    } else {
        HashMap::new()
    };
    
    let new_scenarios = match scenario::load_scenarios_dir(&config_paths.scenarios_dir) {
        Ok(s) => s,
        Err(e) => return vec![IpcResponse::Error { message: e.to_string() }],
    };
    let new_models = match rhd_ai::config::load_models(&config_paths.models_dir, &credentials) {
        Ok(m) => m,
        Err(e) => return vec![IpcResponse::Error { message: e.to_string() }],
    };
    let new_mcp_configs = match mcp_loader::load_mcp_dir(&config_paths.mcp_dir) {
        Ok(m) => m,
        Err(e) => return vec![IpcResponse::Error { message: e.to_string() }],
    };
    let new_projects = match project_loader::load_projects(&config_paths.projects_dir) {
        Ok(p) => p,
        Err(e) => return vec![IpcResponse::Error { message: e.to_string() }],
    };
    
    // 3. Diff MCP configs and manage servers
    let (mcp_to_restart, mcp_to_stop) = {
        let inner = state.inner.read();
        diff_mcp_configs(&inner.mcp_configs, &new_mcp_configs)
    };
    
    // 4. Stop removed MCPs (log PID for manual kill if needed)
    let stopped_count = state.mcp_cache.stop_specific(&mcp_to_stop).await;
    
    // 5. Restart changed MCPs (kill old, will be respawned on next use)
    let restarted_count = state.mcp_cache.restart_specific(&mcp_to_restart).await;
    
    // 6. Update state
    {
        let mut inner = state.inner.write();
        inner.scenarios = new_scenarios;
        inner.models = new_models;
        inner.mcp_configs = new_mcp_configs;
        inner.project_manager = Arc::new(ProjectManager::new(new_projects));
    }
    
    // 7. Return stats
    let inner = state.inner.read();
    vec![IpcResponse::Reloaded {
        scenarios_reloaded: inner.scenarios.len(),
        models_reloaded: inner.models.len(),
        mcp_restarted: restarted_count,
        mcp_stopped: stopped_count,
        projects_reloaded: inner.project_manager.projects().len(),
    }]
}
```

### 9. MCP Diff Logic — Handle Removed MCPs

**File**: `packages/rhd_app/src/daemon.rs`

```rust
fn diff_mcp_configs(
    old: &HashMap<String, McpConfig>,
    new: &HashMap<String, McpConfig>,
) -> (Vec<String>, Vec<String>) {
    let mut to_restart = Vec::new();  // Changed configs
    let mut to_stop = Vec::new();     // Removed configs
    
    // Find changed or new configs
    for (name, new_config) in new {
        let new_key = mcp_cache_key(new_config);
        
        match old.get(name) {
            Some(old_config) => {
                let old_key = mcp_cache_key(old_config);
                if old_key != new_key {
                    to_restart.push(old_key);  // Stop old, new will spawn on use
                }
            }
            None => {
                // New config, will be spawned on first use
            }
        }
    }
    
    // Find removed configs — stop their MCP servers
    for (name, old_config) in old {
        if !new.contains_key(name) {
            to_stop.push(mcp_cache_key(old_config));
        }
    }
    
    (to_restart, to_stop)
}

fn mcp_cache_key(config: &McpConfig) -> String {
    format!(
        "{}:{}:{}",
        config.cmd.as_deref().unwrap_or(""),
        config.args.join(","),
        config.cwd.as_deref().unwrap_or("")
    )
}
```

### 10. MCP Cache Extensions

**File**: `packages/rhd_app/src/mcp_cache.rs`

Add methods to stop/restart specific MCP servers with PID logging:
```rust
impl McpServerCache {
    /// Stop and remove specific MCP servers from cache. Logs PIDs.
    /// Returns count of stopped servers.
    pub async fn stop_specific(&self, keys_to_stop: &[String]) -> usize {
        let mut cache = self.cache.lock().await;
        let mut stopped = 0;
        for key in keys_to_stop {
            if let Some(client) = cache.remove(key) {
                // Log PID if available
                if let Some(pid) = client.pid() {
                    eprintln!("stopping MCP server '{}' (PID: {})", key, pid);
                } else {
                    eprintln!("stopping MCP server '{}'", key);
                }
                if let Err(e) = client.kill().await {
                    eprintln!("failed to kill MCP server '{}': {}", key, e);
                }
                stopped += 1;
            }
        }
        stopped
    }
    
    /// Alias for stop_specific (restart = stop old, new spawns on next use)
    pub async fn restart_specific(&self, keys_to_restart: &[String]) -> usize {
        self.stop_specific(keys_to_restart).await
    }
}
```

### 11. MCP Client PID Access

**File**: `packages/rhd_mcp_client/src/client.rs`

Add method to get PID of spawned MCP process:
```rust
impl McpClient {
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(|c| c.id())
    }
}
```

### 12. Chat Manager Extension

**File**: `packages/rhd_app/src/chat.rs`

Add method to count active streams (for waiting):
```rust
impl ChatManager {
    pub async fn active_stream_count(&self) -> usize {
        self.active_streams.lock().await.len()
    }
}
```

### 13. WebSocket Handler Updates

**File**: `packages/rhd_app/src/ws.rs`

Update WebSocket handlers to acquire `reload_lock.read()` before starting operations:
- `runScenario`: Acquire read lock (blocks silently during reload)
- `sendMessage`: Acquire read lock
- `editMessage`: Acquire read lock

This ensures chats/scenarios wait silently instead of returning errors.

### 14. Frontend Notification (Deferred)

WebSocket events for reload (`ReloadStarted`, `ReloadFinished`) can be added later. Currently no frontend visualization needed.

## File Changes Summary

| File | Changes |
|------|---------|
| `packages/rhd_app/src/ipc/protocol.rs` | Add `Reload` request, `Reloaded` response |
| `packages/rhd_app/src/cli.rs` | Add `Reload` subcommand, `ReloadArgs` |
| `packages/rhd_app/src/client.rs` | Add `send_reload()` function |
| `packages/rhd_app/src/main.rs` | Handle `Command::Reload` |
| `packages/rhd_app/src/daemon.rs` | Refactor state with `Inner` + `RwLock`, add reload handler, diff logic |
| `packages/rhd_app/src/chat.rs` | Add `active_stream_count()`, reload lock gating |
| `packages/rhd_app/src/mcp_cache.rs` | Add `stop_specific()`, `restart_specific()` with PID logging |
| `packages/rhd_app/src/ws.rs` | Add reload lock gating |
| `packages/rhd_mcp_client/src/client.rs` | Add `pid()` method |

## Testing Considerations

1. **Unit tests**:
   - MCP diff logic (changed, added, removed)
   - Reload lock behavior

2. **Integration tests**:
   - Reload with no active executions
   - Reload with active executions (wait behavior)
   - Reload with changed MCP configs
   - Reload with removed MCP configs (verify stopped)
   - Reload with new/removed scenarios

3. **E2E tests**:
   - `rhd reload` command execution
   - Verify configs reloaded
   - Verify MCP servers stopped/restarted correctly
   - Verify PID logged for stopped MCPs

## Edge Cases

1. **Reload during reload**: Second reload request blocks on `reload_lock.write()` until first completes
2. **Config load failure**: Return error, don't update state, release lock
3. **Credentials file missing**: Return error, don't update state
4. **MCP kill failure**: Log error with PID, continue with other MCPs
5. **Empty config dirs**: Handle gracefully (empty HashMaps)
6. **Poisoned lock**: Panic with clear message (indicates bug)
