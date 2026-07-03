# E2E Log File Test Plan

## Goal

Add verification to the existing e2e test ([`run_single_test()`](packages/rhd_test/src/main.rs:150)) that when daemon is started with `--logs`, a log directory is created containing `log.txt` with expected content matching the scenario execution output.

## Current State

- [`run_single_test()`](packages/rhd_test/src/main.rs:150) spawns daemon without `--logs` flag
- [`create_log_dir()`](packages/rhd_app/src/log.rs:86) creates `<logs>/<scenarioName>-<YYYY-MM-DD-HH-MM-SS>[-N]/`
- [`open_log_file()`](packages/rhd_app/src/log.rs:103) creates `log.txt` inside that directory
- [`LogSink`](packages/rhd_app/src/log.rs:22) writes identical content to both stdout and `log.txt`
- Test scenario [`rhd_test`](test_e2e/scenarios/rhd_test/scenario.yaml:1) has 3 steps: `cmd1` (runCommand), `ai1` (aiChat), `out1` (output)

## Changes

### 1. Pass `--logs` to daemon in e2e test

In [`run_single_test()`](packages/rhd_test/src/main.rs:184), create a `logs_dir` inside `daemon_dir` and pass `--logs <path>` to daemon:

```rust
let logs_dir = daemon_dir.path().join("logs");
// ...
let mut daemon = Command::new(&rhd_bin)
    .arg("daemon")
    .arg("--models-dir").arg(&models_dir)
    .arg("--scenarios-dir").arg(&scenarios_dir)
    .arg("--socket").arg(&socket_path)
    .arg("--logs").arg(&logs_dir)       // <-- new
    // ...
```

### 2. After scenario run, find log directory

After `rhd run` completes, read `logs_dir` entries. Expect exactly one subdirectory matching pattern `rhd_test-*`:

```rust
let log_entries = std::fs::read_dir(&logs_dir).unwrap();
let log_dirs: Vec<_> = log_entries
    .filter_map(|e| e.ok())
    .filter(|e| {
        let name = e.file_name().to_string_lossy().to_string();
        name.starts_with("rhd_test-") && e.path().is_dir()
    })
    .collect();
```

### 3. Verify log directory and file exist

Assert:
- Exactly 1 log directory found
- `log.txt` exists inside it

### 4. Verify log.txt content

Read `log.txt` and assert it contains expected blocks in order:

| Expected substring | Source |
|---|---|
| `===== executing scenario =====\nrhd_test` | [`sink.log("executing scenario", ...)`](packages/rhd_app/src/scenario/executor.rs:51) |
| `cmd1: ===== running command =====` | [`sink.log_step(step_name, "running command", ...)`](packages/rhd_app/src/scenario/executor.rs:92) |
| `[STDOUT]` | [`log_command_output()`](packages/rhd_app/src/log.rs:53) |
| `cmd1: ===== command exit code =====` | [`sink.log_step(step_name, "command exit code", ...)`](packages/rhd_app/src/scenario/executor.rs:172) |
| `ai1: ===== AI request =====` | [`sink.log_ai_request()`](packages/rhd_app/src/scenario/executor.rs:250) |
| `model: test-model` | AI request body |
| `----- system prompt -----` | AI request sub-header |
| `----- message -----` | AI request sub-header |
| `ai1: ===== AI response =====` | [`sink.log_step(step_name, "AI response", ...)`](packages/rhd_app/src/scenario/executor.rs:256) |
| `<random_response>` | Mock AI response value |
| `===== output step =====` | [`sink.log("output step", ...)`](packages/rhd_app/src/scenario/executor.rs:69) |
| `AI said: <random_response>` | Resolved output placeholder |

### 5. Verify log.txt matches stdout

The daemon stdout is currently drained and discarded. To compare log file with stdout, capture daemon stdout lines (collected by the drain task) into a shared buffer, then assert `log.txt` content equals the captured stdout.

Implementation:
- Change drain task to collect lines into `Arc<Mutex<String>>` instead of discarding
- After `rhd run` completes, read captured stdout
- Assert `log.txt` content == captured stdout (both written by same `LogSink` calls)

## File Changes

| File | Change |
|---|---|
| `packages/rhd_test/src/main.rs` | Add `--logs` arg to daemon spawn; add log directory/file/content assertions; capture daemon stdout for comparison |

## Risks

- **Timing**: log directory created during scenario execution; must read after `rhd run` exits (already the case)
- **Stdout capture**: drain task currently discards; switching to capture adds memory overhead but tests are short-lived
- **Non-deterministic timestamp**: log dir name contains timestamp; match by prefix `rhd_test-` rather than exact name

## Success Criteria

- E2E test passes `--logs` to daemon
- Exactly one log directory created per test iteration
- `log.txt` exists and contains all expected log blocks
- `log.txt` content matches daemon stdout
- `cargo run -p rhd_test` passes
