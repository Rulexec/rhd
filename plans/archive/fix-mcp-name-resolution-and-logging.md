# Fix MCP Name Resolution and Improve Error Logging

## Problem Summary

User has:
- `mcp/fs.yaml` with `cmd: npx`, `args: [...]`
- Project `mcp.yaml` with `mcp: - name: fs`

Expected: `fs` MCP implicitly uses config from `mcp/fs.yaml` (name from filename).

Actual error:
```
Transport error: Failed to spawn MCP server 'fs': No such file or directory (os error 2)
```

## Root Cause

In [`project_manager.rs:86-92`](packages/rhd_app/src/project_manager.rs:86), when spawning project MCP:

```rust
let mcp_config = McpConfig {
    name: mcp_ref.name.clone(),
    cmd: Some(mcp_ref.name.clone()),  // BUG: It should never use `name` as `cmd`
    args: mcp_ref.args.clone().unwrap_or_default(),
    cwd: None,
    env: mcp_ref.env.clone().unwrap_or_default(),
};
```

The code uses `mcp_ref.name` as the command to execute, instead of looking up the actual MCP config from `mcp/` directory by name.

## Solution

### 1. Pass MCP configs to ProjectManager

Modify `ProjectManager::new()` to accept `HashMap<String, McpConfig>` (loaded from `mcp/` directory).

**Files to modify:**
- [`packages/rhd_app/src/project_manager.rs`](packages/rhd_app/src/project_manager.rs)
- [`packages/rhd_app/src/main.rs`](packages/rhd_app/src/main.rs)
- [`packages/rhd_app/src/daemon.rs`](packages/rhd_app/src/daemon.rs) (for reload)

### 2. Resolve McpRef to actual McpConfig

In `spawn_project_mcp()`, look up `mcp_ref.name` in the MCP configs map:

```rust
let base_config = self.mcp_configs.get(&mcp_ref.name)
    .ok_or_else(|| format!("MCP config '{}' not found in mcp/ directory", mcp_ref.name))?;

let mcp_config = McpConfig {
    name: base_config.name.clone(),
    cmd: base_config.cmd.clone(),
    args: mcp_ref.args.clone().unwrap_or_else(|| base_config.args.clone()),
    cwd: base_config.cwd.clone(),
    env: {
        let mut env = base_config.env.clone();
        if let Some(ref override_env) = mcp_ref.env {
            env.extend(override_env.clone());
        }
        env
    },
};
```

### 3. Improve error messages

In [`transport.rs:36-38`](packages/rhd_mcp_client/src/transport.rs:36), include full command in error:

```rust
let mut child = command.spawn().map_err(|e| {
    McpError::Transport(format!(
        "Failed to spawn MCP server '{}' (cmd: '{}', args: {:?}): {}",
        cmd, cmd, args, e
    ))
})?;
```

### 4. Add logging for MCP spawn

In [`project_manager.rs`](packages/rhd_app/src/project_manager.rs), add `eprintln!` for:
- Successful spawn: `eprintln!("MCP '{}' spawned (id: '{}', cmd: '{}', cwd: {:?}, PID: {:?})", mcp_ref.name, mcp_ref.effective_id(), cmd, cwd, client.pid())`
- Failed spawn: `eprintln!("MCP '{}' spawn failed (id: '{}', cmd: '{}', cwd: {:?}): {}", mcp_ref.name, mcp_ref.effective_id(), cmd, cwd, error_msg)`

Note: Do NOT log args as they may contain secrets.

In [`daemon.rs`](packages/rhd_app/src/daemon.rs), add logging after loading MCP configs:
```rust
eprintln!("loaded {} MCP config(s): {}", mcp_configs.len(), mcp_configs.keys().join(", "));
```

## Implementation Steps

1. **Modify `ProjectManager` struct** to store `mcp_configs: HashMap<String, McpConfig>`
2. **Update `ProjectManager::new()`** signature to accept MCP configs
3. **Fix `spawn_project_mcp()`** to resolve McpRef to actual McpConfig
4. **Update callers** in `main.rs` and `daemon.rs` to pass MCP configs
5. **Improve error message** in `transport.rs` to include full command
6. **Add spawn logging** in `project_manager.rs` (cmd, id, name, cwd — no args)
7. **Add MCP config loading log** in `daemon.rs`
8. **Update tests** in `project_manager.rs` to reflect new signature

## Files to Modify

| File | Changes |
|------|---------|
| `packages/rhd_app/src/project_manager.rs` | Store mcp_configs, resolve McpRef, add logging |
| `packages/rhd_app/src/main.rs` | Pass mcp_configs to ProjectManager::new() |
| `packages/rhd_app/src/daemon.rs` | Pass mcp_configs on reload, add loading log |
| `packages/rhd_mcp_client/src/transport.rs` | Improve spawn error message |
