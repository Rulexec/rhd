# Scenario Control Fixes

## Status

**Last Updated**: 2026-07-03

## Fixed Issues

### Issue 4: WebSocket event name casing mismatch — FIXED

**Problem**: Frontend Zod validation failed with "Invalid discriminator value. Expected 'scenariostarted' | 'stepstarted' | ..." when receiving events like `scenarioResumed`.

**Root Cause**: Backend used `format!("{:?}", exec_event.event).to_lowercase()` which produces all-lowercase event names (e.g., `"scenarioresumed"`), but frontend Zod schema expected camelCase (e.g., `"scenarioResumed"`).

**Fix Applied**:
- `packages/rhd_app/src/ws.rs`: Changed event name generation to use `serde_json::to_string(&exec_event.event)` which produces correct camelCase via serde's `rename_all = "camelCase"` attribute
- `frontend/src/lib/types/ws.ts`: Updated Zod schema literals to camelCase: `scenariostarted` → `scenarioStarted`, `stepstarted` → `stepStarted`, `scenariofinished` → `scenarioFinished`
- `frontend/src/lib/ws.ts`: Updated switch cases to match camelCase event names

### Issue 5: IPC deserialization error after retry — FIXED

**Problem**: After successful retry, `rhd run` showed error: "invalid response from daemon: context error: pointer out of bounds: base 0x1039d19f0 offset -19 not in range"

**Root Cause**: Daemon's `write_message` writes protocol version + length + payload for every message, but client's `run_scenario` loop only read length + payload after the first response. This caused stream misalignment when reading multiple responses (Paused → Success).

**Fix Applied**:
- `packages/rhd_app/src/client.rs`: Added version reading inside the response loop so each response is properly read with its version prefix
- Removed duplicate version read before loop

### Issue 6: Pause messages printed in `rhd run` output — FIXED

**Problem**: Success response of `rhd run` was prepended with "Scenario paused at step 'ai': ..." and "Waiting for resume or abort..." messages.

**Root Cause**: Client printed these messages when handling `IpcResponse::Paused`.

**Fix Applied**:
- `packages/rhd_app/src/client.rs`: Removed `eprintln!` calls in `IpcResponse::Paused` handler, now silently continues loop to read final response

## Original Issues (from earlier work)

### Issue 1: Active scenario not showing in scenarios tab without refresh — FIXED

**Problem**: When a scenario is started via `rhd run`, it doesn't appear in the Active section of the scenarios tab until the page is refreshed.

**Root Cause**: Backend sends `id` and `executionId` as numbers (u64), but frontend stores expect strings. The `activeScenariosList` derived store calls `b.id.localeCompare(a.id)` which throws TypeError on numbers (no such method), causing the derived store computation to fail silently and UI not update.

**Fix Applied**:
- `frontend/src/lib/ws.ts`: Convert all IDs to strings with `String()` when processing WebSocket events (`scenariostarted`, `stepstarted`, `scenarioPaused`, `scenarioResumed`, and `subscribe()` response)
- `frontend/src/lib/stores.ts`: Convert IDs to strings in `setFromList()` method

### Issue 2: Retry button doesn't show loading state — FIXED

**Problem**: When the retry button is clicked, there's no visual indication that a retry request is in progress. The button should be disabled and show a loading indicator.

**Root Cause**: The `PausedScenario.svelte` component calls `retryScenario()` but doesn't track the loading state.

**Fix Applied**:
- Added `retrying` state variable to `PausedScenario.svelte`
- Set to `true` when retry clicked, reset to `false` in finally block
- Button disabled while retrying, text changes to "Retrying..."

### Issue 3: Retry doesn't continue scenario execution — FIXED

**Problem**: Retry button retries the single AI chat request but doesn't continue the scenario execution after the retry succeeds.

**Root Cause**: In `execute_ai_chat_with_tools()`, the `client`, `display_name`, and `api_model` were passed as immutable references and never updated on retry with model override. The retry would `continue` the loop but use the same broken client/model, causing it to fail again silently. The non-MCP path (`execute_ai_chat` without tools) correctly updated `current_model_config` and created a new client each iteration, but the MCP path did not.

**Fix Applied**:
- Changed `execute_ai_chat_with_tools()` to take owned `OpenAiClient`, `String` for display_name and api_model
- Added mutable `current_client`, `current_display_name`, `current_api_model` variables
- On retry with model override, update all three: create new `OpenAiClient` from new config, update display name and API model
- Moved AI request logging inside the loop so it uses current (potentially updated) display name and message
- Updated call site to pass owned values instead of references

## Implementation Plan

### Step 1: Fix active scenario not showing

**Files**: `packages/rhd_app/src/daemon.rs`, `packages/rhd_app/src/ws.rs`, `frontend/src/lib/ws.ts`

1. Verify `scenariostarted` event is emitted when execution starts in `daemon.rs`
2. Check WebSocket handler in `ws.rs` emits the event to connected clients
3. Verify frontend `ws.ts` processes the event and updates `activeScenarios` store
4. Test by starting a scenario and checking if it appears without refresh

### Step 2: Add loading state to retry button

**Files**: `frontend/src/components/PausedScenario.svelte`

1. Add `let retrying = $state(false);` state variable
2. In `handleRetry()`:
   - Set `retrying = true`
   - Call `retryScenario()`
   - Handle success/failure
   - Set `retrying = false` in finally block
3. Update button:
   - Add `disabled={retrying}` attribute
   - Show loading text or spinner when `retrying` is true

### Step 3: Fix retry not continuing scenario execution

**Files**: `packages/rhd_app/src/scenario/ai_chat.rs`, `packages/rhd_app/src/scenario/executor.rs`, `packages/rhd_app/src/execution.rs`

1. Add logging to track retry flow:
   - Log when retry is triggered
   - Log when retry request is made
   - Log when retry succeeds/fails
   - Log when step completes after retry
   - Log when scenario continues to next step

2. Check execution flow after retry:
   - Verify `pause_and_wait` returns correctly
   - Verify the loop continues and retries the request
   - Verify the step completes after successful retry
   - Verify the scenario executor continues to the next step

3. Check for state issues:
   - Verify execution state is correct after pause/resume
   - Check if any state is lost during pause
   - Verify the execution handle is still valid after resume

4. Test the full flow:
   - Start scenario with broken model
   - Wait for pause
   - Click retry (with working model)
   - Verify scenario continues and completes

## Testing

1. Start daemon with `neverFail: true`
2. Run scenario with `rhd run`
3. Verify scenario appears in Active section without refresh
4. Wait for pause (AI error)
5. Verify retry button shows loading state when clicked
6. Click retry with working model
7. Verify scenario continues and completes
8. Verify final result appears in Finished section
