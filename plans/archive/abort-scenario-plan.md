# Abort Scenario Execution Plan

## Goal
Add ability to stop active scenario execution from frontend, with proper cleanup and status tracking.

## Requirements
1. Frontend button to abort active scenario
2. Send abort message to daemon with scenario execution ID
3. Daemon kills current operation (command/AI/MCP)
4. Write `===== ABORTED =====` to log
5. Add status to ScenarioMeta: `executing`, `error`, `success`, `aborted`
6. Show status on frontend for each scenario
7. `rhd run` exits with code 2 and prints `ABORTED` when aborted

## Architecture

### Cancellation Mechanism
Use `tokio::sync::watch<bool>` as abort signal per execution:
- `ExecutionTracker` stores `AbortHandle` (wrapper around `watch::Sender<bool>`) per active execution
- `ExecutionHandle` gets clone of abort receiver
- Long-running operations use `tokio::select!` with `abort_receiver.changed()` to wait for abort signal
- No polling needed — `select!` efficiently waits for either operation completion or abort

### Status Tracking
Add `ScenarioStatus` enum to `rhd_api`:
```rust
pub enum ScenarioStatus {
    Executing,
    Success,
    Error,
    Aborted,
}
```

Add `status` field to `ScenarioMeta`.

### IPC Protocol Changes
Add `IpcResponse::Aborted` variant for client to detect abort.

### WebSocket Protocol Changes
Add `WsRequest::AbortScenario { id: String, execution_id: u64 }`.

## Implementation Steps

### 1. rhd_api Changes
- Add `ScenarioStatus` enum
- Add `status: ScenarioStatus` field to `ScenarioMeta`
- Add `WsRequest::AbortScenario` variant

### 2. ExecutionTracker Changes
- Store `AbortHandle` (wrapper around `watch::Sender<bool>`) per execution
- `start()` returns `ExecutionHandle` with abort receiver clone
- Add `abort(execution_id: u64)` method
- `ActiveExecution` stores abort handle

### 3. ExecutionHandle Changes
- Add `abort_receiver: watch::Receiver<bool>`
- Add `abort_signal()` method returning cloned receiver for `select!`

### 4. Scenario Executor Changes
- Use `tokio::select!` with `abort_signal().changed()` around each action
- Return `ExecuteError::Aborted` if aborted
- Pass abort signal to `execute_run_command` and `execute_ai_chat`

### 5. run_command.rs Changes
- Store `Child` process handle
- Use `tokio::select!` with abort signal while waiting for output
- On abort: call `child.kill().await`
- Return partial result with abort indicator

### 6. ai_chat.rs Changes
- Use `tokio::select!` with abort signal around HTTP calls
- On abort: drop the future (HTTP client will cancel)
- Return `ExecuteError::Aborted`

### 7. Log Changes
- Add `log_aborted()` method to `LogSink`
- Write `===== ABORTED =====` when scenario aborted

### 8. Daemon Changes
- Handle abort in `handle_request`:
  - Call `execution_tracker.abort(id)`
  - Wait for execution to finish
  - Set status to `Aborted` in meta.json
- For WebSocket: add `handle_abort_scenario`

### 9. Client Changes
- Detect `IpcResponse::Aborted` 
- Print `ABORTED` to stdout
- Exit with code 2

### 10. Frontend Changes
- Add `abortScenario(executionId)` to `ws.js`
- Add abort button to `ActiveScenario.svelte`
- Show status badge in `ActiveScenario.svelte` and `FinishedScenario.svelte`
- Color-code status: executing (blue), success (green), error (red), aborted (orange)

## File Changes

### packages/rhd_api/src/lib.rs
- Add `ScenarioStatus` enum
- Add `status` field to `ScenarioMeta`
- Add `AbortScenario` to `WsRequest`

### packages/rhd_app/src/execution.rs
- Add `AbortHandle` struct
- Modify `ActiveExecution` to store abort handle
- Add `abort()` method to `ExecutionTracker`
- Add abort receiver to `ExecutionHandle`
- Add `abort_signal()` method

### packages/rhd_app/src/scenario/error.rs
- Add `Aborted` variant to `ExecuteError`

### packages/rhd_app/src/scenario/executor.rs
- Use `select!` with abort signal around each action
- Return `ExecuteError::Aborted` if aborted

### packages/rhd_app/src/scenario/run_command.rs
- Use `select!` with abort signal
- Kill child process on abort

### packages/rhd_app/src/scenario/ai_chat.rs
- Use `select!` with abort signal around HTTP calls

### packages/rhd_app/src/log.rs
- Add `log_aborted()` method

### packages/rhd_app/src/daemon.rs
- Handle abort in IPC handler
- Set status in `build_scenario_meta`

### packages/rhd_app/src/ws.rs
- Add `handle_abort_scenario`
- Wire up to `WsRequest::AbortScenario`

### packages/rhd_app/src/ipc/protocol.rs
- Add `IpcResponse::Aborted` variant

### packages/rhd_app/src/client.rs
- Handle `IpcResponse::Aborted`
- Print `ABORTED`, return special error for exit code 2

### packages/rhd_app/src/main.rs
- Handle abort error type, exit with code 2

### frontend/src/lib/ws.js
- Add `abortScenario(executionId)` function

### frontend/src/components/ActiveScenario.svelte
- Add abort button
- Show status badge

### frontend/src/components/FinishedScenario.svelte
- Show status badge with color coding

## Risks
1. Killing child processes may leave orphaned grandchildren
2. HTTP request cancellation depends on reqwest behavior
3. Race conditions between abort and normal completion

## Success Criteria
- Frontend button sends abort request
- Daemon kills running command/AI operation
- Log contains `===== ABORTED =====`
- meta.json contains `status: "aborted"`
- Frontend shows status for each scenario
- `rhd run` exits with code 2 and prints `ABORTED`
