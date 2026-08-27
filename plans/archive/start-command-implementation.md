# Start Command Implementation Plan

## Overview

Add a `start` command to `rhd_app` that reads a YAML configuration file and spawns multiple child processes (like `rhd_chat_server` and `rhd_plugin_ai_completions`), forwarding their stdout/stderr with prefixed output and handling graceful shutdown on Ctrl+C.

## Requirements

1. **New `start` command** in `rhd_app` that accepts a single argument: path to YAML config
2. **YAML config format**:
   ```yaml
   children:
     - name: chat_server
       cmd: rhd_chat_server
       cwd: .  # optional, defaults to config file directory
       args:
         - '--db-path'
         - ./rhd_db
     - name: ai_completions
       cmd: rhd_plugin_ai_completions
       cwd: .
       args:
         - '--server-url'
         - 'ws://127.0.0.1:8080'
         - '--config'
         - './rhd.yaml'
   ```
3. **Process management**:
   - Start all child processes
   - Read stdout/stderr line by line
   - Print to parent's stdout/stderr with `[<name>] ` prefix
   - On Ctrl+C, propagate signal to all children
   - Exit when all children have quit
4. **Modify `rhd_chat_server`**:
   - Change `--db-path` to accept a folder path
   - Store database at `<folder>/chats.db`

## Implementation Steps

### Phase 1: Add YAML Config Parsing

**Files to modify:**
- `packages/rhd_app/Cargo.toml` - Add `serde_yaml` dependency
- `packages/rhd_app/src/cli.rs` - Add `Start` command variant
- `packages/rhd_app/src/commands/mod.rs` - Add `start` module
- `packages/rhd_app/src/commands/start.rs` - New file for start command logic

**Config struct:**
```rust
#[derive(Deserialize)]
struct StartConfig {
    ws_port: u16,
    children: Vec<ChildConfig>,
}

#[derive(Deserialize)]
struct ChildConfig {
    name: String,
    cmd: String,
    cwd: Option<String>,
    args: Option<Vec<String>>,
}
```

### Phase 2: Implement Child Process Spawning

**Key components:**
1. Resolve `cwd` relative to config file directory if not absolute
2. Spawn each child process with `Stdio::piped()` for stdout/stderr
3. Create async tasks to read stdout/stderr line by line
4. Prefix each line with `[<name>] ` and print to parent's stdout/stderr

**Implementation approach:**
- Use `tokio::process::Command` for async process management
- Use `tokio::io::BufReader` with `lines()` for line-by-line reading
- Spawn separate tasks for stdout and stderr of each child

### Phase 3: Signal Handling

**Requirements:**
- Catch SIGINT (Ctrl+C) using `tokio::signal::ctrl_c()`
- On signal, send SIGTERM to all child processes
- Wait for all children to exit
- Exit parent process

**Implementation:**
- Store child process handles in a `Vec<tokio::process::Child>`
- Create a shutdown signal task
- On shutdown, iterate through children and send kill signal
- Wait for all children to complete

### Phase 4: Modify rhd_chat_server db-path

**Files to modify:**
- `packages/rhd_chat_server/src/config.rs` - Update help text
- `packages/rhd_chat_server/src/server.rs` - Construct full DB path

**Changes:**
1. Update `--db-path` description to indicate it's a folder path
2. In `server::run()`, construct full path: `<db_path>/chats.db`
3. Ensure the directory exists (create if needed)

### Phase 5: Integration and Testing

**Testing scenarios:**
1. Config with relative `cwd` paths
2. Config with absolute `cwd` paths
3. Config without `cwd` (should default to config directory)
4. Graceful shutdown on Ctrl+C
5. Child process failure handling
6. Verify `rhd_chat_server` creates DB in correct location

## Dependencies to Add

**packages/rhd_app/Cargo.toml:**
```toml
serde_yaml = "0.9"
```

## File Structure

```
packages/rhd_app/src/
├── cli.rs                    # Add Start command
├── main.rs                   # Add Start command dispatch
├── commands/
│   ├── mod.rs               # Add start module
│   └── start.rs             # New: start command implementation
```

## Key Implementation Details

### Config Path Resolution
```rust
let config_dir = std::path::Path::new(&config_path)
    .parent()
    .unwrap_or(std::path::Path::new("."));

let cwd = child.cwd
    .as_ref()
    .map(|c| resolve_path(config_dir, c))
    .unwrap_or_else(|| config_dir.to_path_buf());
```

### Process Spawning Pattern
```rust
let mut child = tokio::process::Command::new(&child_config.cmd)
    .args(&child_config.args.unwrap_or_default())
    .current_dir(cwd)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
```

### Output Forwarding Pattern
```rust
let stdout = child.stdout.take().unwrap();
let name = child_config.name.clone();
tokio::spawn(async move {
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        println!("[{}] {}", name, line);
    }
});
```

### Signal Handling Pattern
```rust
tokio::select! {
    _ = ctrl_c() => {
        // Send SIGTERM to all children
        for child in children.iter_mut() {
            let _ = child.kill().await;
        }
    }
    // Wait for all children to exit
    results = futures::future::join_all(child_futures) => {
        // All children exited normally
    }
}
```

## Migration Notes

**rhd_chat_server db-path change:**
- Old behavior: `--db-path ./rhd_db/chats.db` (full file path)
- New behavior: `--db-path ./rhd_db` (folder path, creates `chats.db` inside)
- This is a breaking change for existing CLI usage
- Update documentation accordingly

## Success Criteria

1. `rhd start config.yaml` successfully starts all configured processes
2. Output from each process is prefixed with `[name] `
3. Ctrl+C gracefully shuts down all processes
4. `rhd_chat_server --db-path ./folder` creates `./folder/chats.db`
5. Relative paths in config are resolved relative to config file location
