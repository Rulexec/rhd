# Config File + Logs Directory Plan

## Goal

1. Add `--config` option to `rhd daemon` (default `./rhd.yaml`). Config holds equivalents of CLI args (scenarios dir, models dirs, logs dir, default model). Socket path stays CLI-only. **Remove `--verbose` flag entirely** — new-format output always emitted to daemon stdout.
2. Add `--logs` CLI arg and `logs` config field. When set, each scenario run writes a log file into `<logs>/<scenarioName>-YYYY-MM-DD-HH-MM-SS[-N]/log.txt`.
3. Replace `[verbose] ...` console output with visually distinct block headers written to **stdout**:
   - Step name prepended to every non-output step header: `<stepName>: <header>`.
   - `output` action type: `===== output step =====\n{content}` (no step name prefix).
   - Other steps:
     - `<step>: ===== running command =====\n{cmd} {args}`
     - `<step>: ===== command output =====\n` followed by interleaved lines prefixed `[STDOUT]` / `[STDERR]` in arrival order.
     - `<step>: ===== command exit code =====\n{code}`
     - `<step>: ===== AI request =====\nmodel: {name}\n----- system prompt -----\n{prompt}\n----- message -----\n{msg}`
     - `<step>: ===== AI response =====\n{response}`
     - `<step>: ===== AI request failed =====\n{err}`
     - `<step>: ===== command spawn error =====\n{err}`
   - Scenario-level:
     - `===== executing scenario =====\n{name}`
     - `===== unknown scenario =====\n{name}`
     - `===== scenario execution error =====\n{err}`
4. `stdoutStderr` field in `StepResult` currently stores a single combined `String`. To preserve arrival-order interleaving with `[STDOUT]`/`[STDERR]` prefixes, change storage to `Vec<OutputLine>` where `OutputLine` is an enum `{ Stdout(String), Stderr(String) }`. The combined string is still derivable for placeholder `%step.stdoutStderr%` by stripping prefixes and concatenating.

## Architecture

### Config file (`rhd.yaml`)

```yaml
scenariosDir: scenarios
modelsDir: models
defaultModel: null
logs: null
```

- Parsed with serde_yaml; field names camelCase via `#[serde(rename_all = "camelCase")]` + `deny_unknown_fields`.
- CLI args override config values (config = defaults, CLI = overrides).
- Socket path NOT in config (per requirement).
- No `verbose` field (removed).

### Log folder naming

- Base: `<scenarioName>-<YYYY-MM-DD-HH-MM-SS>`
- Collision suffix: if folder exists, append `-2`, `-3`, ... until free.
- Inside: single `log.txt` with full output for that run (same format as stdout).

### Output sink

Introduce `LogSink` that writes to:
- stdout (always, daemon process)
- `log.txt` (only when logs dir configured; same format duplicated)

Executor functions receive `&mut LogSink` instead of `verbose: bool`.

### E2E test startup synchronization

`rhd_test` spawns daemon as child process. Daemon already prints `listening on {sock}` to stdout when ready. Test will:
1. Spawn daemon with `.stdout(Stdio::piped())`.
2. Read stdout line-by-line until a line starts with `listening on ` (or contains the socket path).
3. Once start message received, spawn a background task to drain/discard further stdout (so pipe buffer never fills).
4. Proceed with scenario execution.

No new IPC protocol needed; reuses existing startup message.

## Implementation Steps

### 1. Add config module
- New file `packages/rhd_app/src/config.rs`.
- `DaemonConfig` struct with serde camelCase + deny_unknown_fields. Fields: `scenarios_dir: PathBuf`, `models_dir: PathBuf`, `default_model: Option<String>`, `logs: Option<PathBuf>`.
- `load_config(path: &Path) -> Result<DaemonConfig>`: reads file, parses YAML. Missing file at default path (`rhd.yaml`) → use built-in defaults. Missing file at explicit `--config` path → error.
- Built-in defaults: `scenariosDir="scenarios"`, `modelsDir="models"`, `defaultModel=None`, `logs=None`.

### 2. Update CLI (`cli.rs`)
- Add to `DaemonArgs`:
  - `#[arg(long, default_value = "rhd.yaml")] pub config: PathBuf`
  - `#[arg(long)] pub logs: Option<PathBuf>`
- Remove `--verbose` field.
- Change other fields (`models_dir`, `scenarios_dir`, `default_model`) to `Option<...>` and drop `default_value` attributes so we can detect "user provided" vs "use config/default".
- Move dir-existence checks out of `DaemonArgs::validate()` (they run after config merge in main).

### 3. Merge config + CLI in `main.rs`
- Load config from `args.config` (if file exists; if default path missing → defaults; if explicit path missing → error).
- Effective values: CLI arg if `Some`, else config value, else built-in default.
- Validate final dirs exist.
- Pass `logs: Option<PathBuf>` into daemon.

### 4. Create `LogSink` (new file `packages/rhd_app/src/log.rs`)
```rust
pub struct LogSink {
    file: Option<BufWriter<File>>,
}

impl LogSink {
    pub fn new(file: Option<File>) -> Self {
        Self { file: file.map(BufWriter::new) }
    }

    /// Write a block: "===== {header} =====\n{body}\n" to stdout and optionally file.
    pub fn log(&mut self, header: &str, body: &str) {
        let block = format!("===== {header} =====\n{body}\n");
        print!("{block}");
        let _ = std::io::stdout().flush();
        if let Some(f) = &mut self.file {
            let _ = f.write_all(block.as_bytes());
            let _ = f.flush();
        }
    }

    /// Write a block with step-name prefix: "{step}: ===== {header} =====\n{body}\n"
    pub fn log_step(&mut self, step: &str, header: &str, body: &str) {
        let block = format!("{step}: ===== {header} =====\n{body}\n");
        print!("{block}");
        let _ = std::io::stdout().flush();
        if let Some(f) = &mut self.file {
            let _ = f.write_all(block.as_bytes());
            let _ = f.flush();
        }
    }

    /// Write command output block with interleaved [STDOUT]/[STDERR] lines.
    pub fn log_command_output(&mut self, step: &str, lines: &[OutputLine]) {
        let mut body = String::new();
        for line in lines {
            match line {
                OutputLine::Stdout(s) => body.push_str(&format!("[STDOUT] {s}")),
                OutputLine::Stderr(s) => body.push_str(&format!("[STDERR] {s}")),
            }
        }
        self.log_step(step, "command output", &body);
    }

    /// Write AI request block with split system prompt and message sub-headers.
    pub fn log_ai_request(&mut self, step: &str, model: &str, system_prompt: &str, message: &str) {
        let mut body = format!("model: {model}\n");
        body.push_str("----- system prompt -----\n");
        body.push_str(system_prompt);
        if !system_prompt.ends_with('\n') { body.push('\n'); }
        body.push_str("----- message -----\n");
        body.push_str(message);
        if !message.ends_with('\n') { body.push('\n'); }
        self.log_step(step, "AI request", &body);
    }
}
```

### 5. Update `StepResult` and placeholder module
- In `packages/rhd_app/src/scenario/placeholder.rs`:
  - Add `pub enum OutputLine { Stdout(String), Stderr(String) }`.
  - Change `StepResult.stdout_stderr: String` → `StepResult.stdout_stderr: Vec<OutputLine>`.
  - Placeholder `%step.stdoutStderr%` resolves by concatenating inner strings (stripping `[STDOUT]`/`[STDERR]` prefixes) in order, preserving original line content.
  - `%step.stdout%` and `%step.stderr%` unchanged (still `String`).

### 6. Log folder creation helper
- New function in `log.rs`:
  ```rust
  pub fn create_log_dir(logs_root: &Path, scenario_name: &str, now: SystemTime) -> io::Result<PathBuf>
  ```
- Format timestamp `YYYY-MM-DD-HH-MM-SS` using `chrono` (add dep).
- Loop: try base name; if exists, try `-2`, `-3`, ...
- Create dir, open `log.txt` inside, return path.

### 7. Update `executor.rs`
- Change signatures: `execute_scenario`, `execute_run_command`, `execute_ai_chat` take `sink: &mut LogSink` instead of `verbose: bool`.
- Remove all `if verbose { ... }` gates; always emit via `sink.log*()`.
- For `runCommand`:
  - Collect stdout/stderr lines into `Vec<OutputLine>` as they arrive (modify channel payload from `(String, String)` to `OutputLine`).
  - After child exits, call `sink.log_command_output(step, &lines)`.
  - Also log exit code via `sink.log_step(step, "command exit code", &exit_code.to_string())`.
- For `aiChat`:
  - Log request via `sink.log_ai_request(step, model_name, &system_prompt, &message)`.
  - Log response via `sink.log_step(step, "AI response", &response)`.
  - Log failure via `sink.log_step(step, "AI request failed", &err.to_string())`.
- For `output` action:
  - Log via `sink.log("output step", &resolved)` (no step prefix).
- Scenario-level:
  - `sink.log("executing scenario", scenario_name)` at start.
- Errors:
  - `sink.log("scenario execution error", &err.to_string())` in daemon on error path.
  - `sink.log("unknown scenario", &name)` in daemon for unknown name.

### 8. Update `daemon.rs`
- `DaemonState`: remove `verbose`, add `logs: Option<PathBuf>`.
- `run_daemon` signature: drop `verbose`, add `logs: Option<PathBuf>`.
- In `handle_request`:
  - Create `LogSink` at top of handler (before any logging).
  - If `logs` set, call `create_log_dir` to make folder + open `log.txt`; pass `Some(file)` to `LogSink::new`.
  - Pass `&mut sink` to `execute_scenario`.
- Remove all `if state.verbose { eprintln!(...) }` calls; replace with `sink.log(...)`.

### 9. Update `main.rs`
- Remove `args.verbose` usage.
- Pass `logs` through to `run_daemon`.

### 10. Update `rhd_test`
- When spawning daemon, use `.stdout(Stdio::piped())`.
- Read stdout line-by-line until a line starts with `listening on ` (or contains the socket path).
- Once start message received, spawn a background thread/task to drain remaining stdout (so pipe buffer never fills and daemon never blocks on write).
- Proceed with scenario execution.
- Update any assertions that matched `[verbose]` strings (grep to confirm).

### 11. Update documentation
- `AI.md`: document `--config`, config schema, `--logs`, log folder naming, new console format, removal of `--verbose`.
- `README.md`: same updates (CLI usage, config example).

### 12. Tests
- Unit test for `create_log_dir` collision logic (base, `-2`, `-3`).
- Unit test for `OutputLine` → `%step.stdoutStderr%` placeholder resolution.
- E2E: verify `log.txt` content matches stdout format when `--logs` set.

## File Changes

| File | Change |
|---|---|
| `packages/rhd_app/Cargo.toml` | Add `chrono` dep |
| `packages/rhd_app/src/config.rs` | New: `DaemonConfig`, `load_config()` |
| `packages/rhd_app/src/log.rs` | New: `LogSink`, `OutputLine`, `create_log_dir()` |
| `packages/rhd_app/src/cli.rs` | Add `--config`, `--logs`; remove `--verbose`; make other fields `Option` |
| `packages/rhd_app/src/main.rs` | Load config, merge with CLI, pass `logs` to daemon, drop `verbose` |
| `packages/rhd_app/src/daemon.rs` | Accept `logs`, create per-run log dir + `LogSink`; drop `verbose` |
| `packages/rhd_app/src/scenario/executor.rs` | Replace `verbose: bool` with `&mut LogSink`; new format; use `OutputLine` |
| `packages/rhd_app/src/scenario/placeholder.rs` | Add `OutputLine` enum; change `stdout_stderr` field; update resolution |
| `packages/rhd_app/src/scenario/mod.rs` | Re-export `OutputLine` if needed |
| `packages/rhd_test/src/main.rs` | Parse daemon stdout for start message, then drain; update `[verbose]` assertions if any |
| `AI.md` | Document new features |
| `README.md` | Document new features |

## Risks

- **E2E test breakage**: output format change + stdout vs stderr switch. Mitigation: grep test code for `[verbose]` and stdout assumptions before implementing.
- **Blocking file I/O in async executor**: `LogSink` writes are small; use `BufWriter` + flush. Acceptable for now.
- **Config merge complexity**: making CLI fields `Option` is cleanest; requires care in `validate()` to run after merge.
- **`stdoutStderr` placeholder backward compat**: changing from `String` to `Vec<OutputLine>` requires updating placeholder resolution to concatenate. Any external consumers of `StepResult.stdout_stderr` must be updated.
- **Daemon stdout pipe fill**: if test does not drain stdout after start message, daemon may block on `print!()`. Mitigation: spawn drain thread immediately after detecting start line.

## Success Criteria

- `rhd daemon --config rhd.yaml` loads config; missing file at default path uses defaults.
- CLI args override config values.
- `--logs ./logs` creates `<scenarioName>-<timestamp>[-N]/log.txt` per run.
- Daemon stdout uses `===== header =====\nbody` format with step-name prefixes; no `[verbose]` text.
- `output` action logs without step-name prefix.
- AI request log splits system prompt and message with `----- system prompt -----` / `----- message -----` sub-headers.
- `stdoutStderr` placeholder still resolves correctly (concatenated lines).
- `--verbose` flag removed; config has no `verbose` field.
- E2E tests pass; daemon stdout parsed for start message, then drained.
- `cargo build`, `cargo test` pass.
- `AI.md` and `README.md` updated.
