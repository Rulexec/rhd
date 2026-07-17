# Tracing Migration Plan

## Overview

Migrate daemon operational logs from `eprintln!`/`println!` to `tracing` with structured logging and spans for better debuggability.

**Scope**: Only daemon operational logs (not file-based execution logs in `LogSink`).

## Dependencies

Add to workspace `Cargo.toml`:
```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

Add to `packages/rhd_app/Cargo.toml`:
```toml
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
```

## Initialization

In `main.rs`, initialize tracing subscriber at the start of `main()`:
```rust
use tracing_subscriber::{fmt, EnvFilter};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    
    // ... rest of main
}
```

This allows filtering via `RUST_LOG` environment variable:
- `RUST_LOG=info` - default info level
- `RUST_LOG=debug` - debug level
- `RUST_LOG=rhd_app=trace` - trace level for rhd_app only

## Files to Modify

### 1. `daemon.rs` - Core daemon operations

**Spans to add**:
- `run_daemon()` - root span with socket path, ws_port
- `handle_connection()` - span with connection context
- `handle_request()` - span with request type
- `handle_run_scenario()` - span with scenario name, cwd
- `handle_reload()` - span for reload operation

**Replace**:
- `eprintln!("no scenarios loaded")` → `warn!("no scenarios loaded")`
- `eprintln!("loaded {} scenarios: {}", count, names)` → `info!(count, %names, "loaded scenarios")`
- `println!("listening on {}", path)` → `info!(%path, "daemon listening")`
- `eprintln!("accept error: {err}")` → `error!(%err, "accept error")`
- `eprintln!("received SIGTERM")` → `info!("received SIGTERM, shutting down")`
- `eprintln!("failed to convert stream")` → `error!(%err, "failed to convert stream")`
- `eprintln!("connection error")` → `error!(%err, "connection error")`
- `eprintln!("failed to write meta.json")` → `error!(%err, "failed to write meta.json")`

### 2. `ws.rs` - WebSocket server

**Spans to add**:
- `run_ws_server()` - span with address
- `handle_ws_connection()` - span with peer address

**Replace**:
- `println!("WebSocket listening on {}", addr)` → `info!(%addr, "WebSocket listening")`
- `eprintln!("WebSocket connection error")` → `error!(%peer_addr, %err, "WebSocket connection error")`

### 3. `project_manager.rs` - MCP server management

**Spans to add**:
- `spawn_mcp_server()` - span with MCP name, id, cmd, cwd

**Replace**:
- `eprintln!("MCP '{}' spawned")` → `info!(%name, %id, %cmd, ?cwd, ?pid, "MCP server spawned")`
- `eprintln!("MCP '{}' spawn failed")` → `error!(%name, %id, %cmd, ?cwd, %error, "MCP server spawn failed")`

### 4. `mcp_cache.rs` - MCP server cache

**Spans to add**:
- `stop_specific()` - span with keys to stop
- `restart_specific()` - span with keys to restart

**Replace**:
- `eprintln!("stopping MCP server")` → `info!(%key, ?pid, "stopping MCP server")`
- `eprintln!("failed to kill MCP server")` → `error!(%key, %err, "failed to kill MCP server")`

### 5. `notifications.rs` - Desktop notifications

**Replace**:
- `eprintln!("terminal-notifier not available")` → `warn!("terminal-notifier not available, falling back to osascript")`

### 6. `main.rs` - Entry point

**Keep as-is**: Client-side `println!`/`eprintln!` for user-facing output (not daemon logs).

## Span Structure Example

```rust
#[tracing::instrument(level = "info", skip(state), fields(scenario = %name, cwd = %cwd))]
fn handle_run_scenario(
    name: String,
    cwd: String,
    // ...
) -> Vec<IpcResponse> {
    // All logs inside this function will have scenario and cwd in context
    info!("starting scenario execution");
    // ...
}
```

## Benefits

1. **Structured output**: JSON or formatted output with timestamps, levels, spans
2. **Filtering**: Control verbosity via `RUST_LOG` env var
3. **Context propagation**: Spans automatically add context to nested logs
4. **Future-proof**: Easy to add more debug logs with full context

## Testing

1. Run daemon with `RUST_LOG=info` - should see structured info logs
2. Run with `RUST_LOG=debug` - should see debug logs
3. Run with `RUST_LOG=rhd_app=trace` - should see trace logs for rhd_app only
4. Verify file-based execution logs (LogSink) remain unchanged
