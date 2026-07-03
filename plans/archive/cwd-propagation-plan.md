# CWD Propagation Plan

## Goal

Make `rhd run` send its current working directory to the daemon, so commands execute in the same directory where `rhd run` was invoked. Support `%<cmdActionName>.cwd%` placeholder to access the cwd used for each command action.

## Current State

- `IpcRequest::RunScenario { name: String }` — no cwd field
- `RunCommandAction.working_dir: Option<String>` — optional explicit cwd
- `execute_run_command()` uses `working_dir` if set, otherwise inherits daemon's cwd
- No `cwd` field in `StepResult`, no `%stepName.cwd%` placeholder

## Changes

### 1. IPC Protocol (`packages/rhd_app/src/ipc/protocol.rs`)

Add `cwd: String` to `RunScenario`:

```rust
pub enum IpcRequest {
    RunScenario { name: String, cwd: String },
}
```

### 2. Client (`packages/rhd_app/src/client.rs`)

Get current dir and send in request:

```rust
let cwd = std::env::current_dir()
    .map(|p| p.to_string_lossy().to_string())
    .unwrap_or_default();

let request = IpcRequest::RunScenario {
    name: name.to_string(),
    cwd,
};
```

### 3. Daemon (`packages/rhd_app/src/daemon.rs`)

Pass cwd to executor:

```rust
IpcRequest::RunScenario { name, cwd } => {
    // ...
    let result = tokio::runtime::Handle::current().block_on(execute_scenario(
        scenario,
        &state.models,
        state.default_model.as_deref(),
        state.verbose,
        &cwd,  // new parameter
    ));
}
```

### 4. Executor (`packages/rhd_app/src/scenario/executor.rs`)

- Add `client_cwd: &str` parameter to `execute_scenario()`
- Pass to `execute_run_command()`
- Use `client_cwd` as default when `working_dir` not set
- Store resolved cwd in `StepResult`

```rust
pub async fn execute_scenario(
    scenario: &Scenario,
    models: &HashMap<String, ModelConfig>,
    default_model: Option<&str>,
    verbose: bool,
    client_cwd: &str,
) -> Result<ExecuteOutput, ExecuteError>

async fn execute_run_command(
    cmd: &super::RunCommandAction,
    context: &ExecutionContext,
    verbose: bool,
    client_cwd: &str,
) -> StepResult {
    // ...
    let resolved_cwd = match &cmd.working_dir {
        Some(dir) => resolve_placeholders(dir, context),
        None => client_cwd.to_string(),
    };
    command.current_dir(&resolved_cwd);
    // ...
    StepResult {
        // ...
        cwd: Some(resolved_cwd),
    }
}
```

### 5. Placeholder (`packages/rhd_app/src/scenario/placeholder.rs`)

Add `cwd` field to `StepResult` and resolve `%stepName.cwd%`:

```rust
pub struct StepResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
    pub message: Option<String>,
    pub cwd: Option<String>,  // new field
}

fn resolve_single(placeholder: &str, context: &ExecutionContext) -> String {
    // ...
    match field {
        // ...
        "cwd" => result.cwd.clone().unwrap_or_default(),
        _ => String::new(),
    }
}
```

### 6. E2E Test (`packages/rhd_test/src/main.rs`)

**Run daemon and `rhd run` in separate working directories** to actually verify cwd propagation:

```rust
// Create separate dirs for daemon and client
let daemon_dir = tempfile::tempdir().unwrap();
let client_dir = tempfile::tempdir().unwrap();

// Daemon runs in daemon_dir
let mut daemon = Command::new(&rhd_bin)
    .arg("daemon")
    // ...
    .current_dir(&daemon_dir)  // daemon's cwd
    // ...

// Client runs in client_dir (different from daemon_dir)
let run_output = Command::new(&rhd_bin)
    .arg("run")
    .arg("rhd_test")
    .current_dir(&client_dir)  // client's cwd - this should be used for commands
    // ...
```

Modify `create_temp_script()` to output cwd:

```rust
let script_content = format!(
    r#"#!/bin/sh
echo "{}"
pwd
exit {}
"#,
    script_output, script_exit_code
);
```

Add validation for cwd in AI request:

```rust
// The cwd in AI request should be client_dir, NOT daemon_dir
let expected_cwd = client_dir.path().to_string_lossy();
if !req.user_content.contains(&expected_cwd) {
    log.push_str(&format!(
        "  FAIL: user message does not contain client cwd '{}'\n",
        expected_cwd
    ));
    failed = true;
} else {
    log.push_str("  PASS: user message contains client cwd\n");
}

// Also verify it's NOT the daemon's cwd
let daemon_cwd = daemon_dir.path().to_string_lossy();
if req.user_content.contains(daemon_cwd.as_ref()) && daemon_cwd != expected_cwd {
    log.push_str(&format!(
        "  FAIL: user message contains daemon cwd instead of client cwd\n"
    ));
    failed = true;
}
```

### 7. E2E Scenario (`test_e2e/scenarios/rhd_test/scenario.yaml`)

Update to pass cwd to AI:

```yaml
- type: aiChat
  name: ai1
  model: test_model
  systemPrompt: "You received: %cmd1.stdout%"
  message: "Exit code was %cmd1.exitCode%, success=%cmd1.success%, cwd=%cmd1.cwd%"
```

## File Changes Summary

| File | Change |
|------|--------|
| `packages/rhd_app/src/ipc/protocol.rs` | Add `cwd: String` to `RunScenario` |
| `packages/rhd_app/src/client.rs` | Get and send cwd |
| `packages/rhd_app/src/daemon.rs` | Pass cwd to executor |
| `packages/rhd_app/src/scenario/executor.rs` | Accept `client_cwd`, use as default, store in result |
| `packages/rhd_app/src/scenario/placeholder.rs` | Add `cwd` field, resolve placeholder |
| `packages/rhd_test/src/main.rs` | Run daemon/client in separate dirs, script outputs cwd, validate cwd in AI request |
| `test_e2e/scenarios/rhd_test/scenario.yaml` | Include `%cmd1.cwd%` in message |

## Success Criteria

- `rhd run` sends its cwd to daemon
- Commands execute in client's cwd by default (when `cwd` not set in scenario)
- `%<cmdActionName>.cwd%` resolves to the cwd used for that command
- E2E test passes: daemon and client run in different dirs, random_cmd.sh outputs cwd, AI receives client's cwd (not daemon's), test validates it matches client_dir
