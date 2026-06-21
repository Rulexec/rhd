# Verbose Logging Feature Plan

## Goal
Add `--verbose` CLI flag to daemon that logs all executed commands, their outputs, AI requests sent, AI responses received, and all errors. Off by default.

## Architecture

### Flag Flow
```
CLI (DaemonArgs.verbose) 
  → run_daemon(verbose) 
  → DaemonState { verbose } 
  → execute_scenario(..., verbose) 
  → execute_run_command(..., verbose) / execute_ai_chat(..., verbose)
```

### Logging Points
1. **runCommand**: Log command + args before execution, log exit code + stdout/stderr after
2. **aiChat**: Log model + system prompt + message before API call, log response after
3. **Errors**: Log all errors (ExecuteError variants, connection errors, command failures)
4. **output**: No logging needed (just placeholder resolution)

### Output Format
Use `eprintln!` with `[verbose]` prefix for consistency with existing daemon logging.

## Implementation Steps

### 1. Add `--verbose` flag to CLI
**File**: `packages/rhd_app/src/cli.rs`
- Add `#[arg(long, default_value_t = false)] pub verbose: bool` to `DaemonArgs`

### 2. Pass verbose through daemon
**File**: `packages/rhd_app/src/daemon.rs`
- Add `verbose: bool` parameter to `run_daemon()`
- Add `verbose: bool` field to `DaemonState`
- Pass `state.verbose` to `execute_scenario()` call
- Log connection errors when verbose enabled

### 3. Update executor signature
**File**: `packages/rhd_app/src/scenario/executor.rs`
- Add `verbose: bool` parameter to `execute_scenario()`
- Add `verbose: bool` parameter to `execute_run_command()`
- Add `verbose: bool` parameter to `execute_ai_chat()`

### 4. Add command logging
**File**: `packages/rhd_app/src/scenario/executor.rs` - `execute_run_command()`
- Before execution: log resolved command + args
- After execution: log exit code, stdout, stderr
- On command spawn failure: log error

### 5. Add AI logging
**File**: `packages/rhd_app/src/scenario/executor.rs` - `execute_ai_chat()`
- Before API call: log model name, system prompt, message
- After API call: log response content
- On AI error: log full error details

### 6. Add error logging
**File**: `packages/rhd_app/src/scenario/executor.rs`
- Log all `ExecuteError` variants when they occur (UnknownModel, NoDefaultModel, AiFailed)
- Log command execution failures (non-zero exit codes treated as errors in verbose mode)

**File**: `packages/rhd_app/src/daemon.rs`
- Log connection handling errors
- Log scenario execution errors

### 7. Update main.rs
**File**: `packages/rhd_app/src/main.rs`
- Pass `args.verbose` to `run_daemon()` call

### 8. Update tests (if needed)
- Check if `execute_scenario()` is called directly in tests
- Update test calls to pass `verbose: false`

## File Changes Summary
- `packages/rhd_app/src/cli.rs`: Add verbose field
- `packages/rhd_app/src/daemon.rs`: Add verbose parameter + state field + error logging
- `packages/rhd_app/src/scenario/executor.rs`: Add verbose parameter + logging + error logging
- `packages/rhd_app/src/main.rs`: Pass verbose flag

## Risks
- Verbose output may contain sensitive data (API keys in prompts, command output)
- Large outputs may clutter logs
- Mitigation: Document that verbose mode is for debugging only

## Success Criteria
- `rhd daemon --verbose` logs all commands, outputs, AI requests/responses, and errors
- `rhd daemon` (no flag) produces no extra output
- Existing tests pass
- E2E tests still work
