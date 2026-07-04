# Scenario Execution

## Purpose
Core feature: daemon executes named scenarios (action chains) on behalf of client requests. Scenarios are YAML-defined sequences of commands, AI calls, and output steps.

## How It Works

### Running a Scenario
1. User runs `rhd run <scenario_name>` from any directory
2. Client connects to daemon via Unix socket (`$HOME/rhd.sock` by default)
3. Client sends its current working directory (CWD) to daemon
4. Daemon loads scenario from `scenarios/<name>/scenario.yaml`
5. Daemon executes actions sequentially, resolving placeholders between steps
6. Final `output` step produces text result sent back to client
7. Client prints result and exits (code 0 on success, 1 on error, 2 on abort)

### Scenario Definition
Scenarios live in `scenarios/<name>/scenario.yaml`. The directory name is the scenario identifier (no `name` field needed in YAML).

**Action types:**
- `runCommand` — execute shell command, capture exit code + stdout/stderr
- `aiChat` — call AI model with system prompt + message, get response
- `output` — produce final text using placeholders from previous steps

**Placeholders:** `%stepName.field%` resolves to step results:
- `exitCode`, `stdout`, `stderr`, `stdoutStderr`, `success`, `cwd` (for runCommand)
- `message` (for aiChat)
- `cwd` (for runCommand — the working directory used)

**Environment variables:** `$VAR` syntax in all string fields, substituted at load time.

### Execution Behavior
- `runCommand` never fails the scenario on non-zero exit — exit code captured as data
- Missing placeholders resolve to empty string (no errors)
- `cwd` defaults to client's CWD; scenario can override per-command
- `stdoutStderr` preserves line-by-line interleaving of stdout/stderr in arrival order
- `stdoutStderr` available even on command failure (so AI can diagnose errors)

### Scenario Control (Pause/Resume/Retry)
When `neverFail: true` is set in config and WebSocket server is enabled:
- AI errors pause execution instead of failing the scenario
- Frontend shows paused scenario with error details
- User can retry (optionally switching model) or abort
- `rhd run` client waits during pause, receives final result after resume/abort
- Desktop notifications alert user when scenario pauses

### Abort
- Frontend can abort active scenario execution
- Daemon kills running operation (command/AI/MCP call)
- Log writes `===== ABORTED =====` marker
- `meta.json` records `status: "aborted"`
- `rhd run` exits with code 2 and prints `ABORTED`

### Model Aliases
- YAML aliases: `models/medium.yaml` with `alias: gpt4` resolves `medium` → `gpt4`
- CLI override: `rhd run example --modelAlias medium=gpt4` replaces model names at runtime
- Two-stage resolution: YAML aliases first, then CLI overrides
- Logs and meta.json show final resolved model name

## Status Tracking
Each scenario run records status in `meta.json`:
- `executing` — currently running
- `success` — completed normally
- `error` — failed with error
- `aborted` — user aborted

## Key Files
- Scenario loading: `packages/rhd_app/src/scenario/loader.rs`
- Execution engine: `packages/rhd_app/src/scenario/executor.rs`
- Placeholder resolution: `packages/rhd_app/src/scenario/placeholder.rs`
- Command execution: `packages/rhd_app/src/scenario/run_command.rs`
- AI chat execution: `packages/rhd_app/src/scenario/ai_chat.rs`
- IPC protocol: `packages/rhd_app/src/ipc/protocol.rs`
- Client: `packages/rhd_app/src/client.rs`
- Daemon handler: `packages/rhd_app/src/daemon.rs`
