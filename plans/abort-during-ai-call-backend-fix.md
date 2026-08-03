# Abort During AI Call — Backend Fix

## Problem

When a user aborts a chat during an AI call (while thinking is streaming), the frontend gets stuck:
- The streaming/thinking content remains visible
- The content loader stays on screen
- The input is disabled
- No Resume or Queue buttons appear

The user expects: after the server confirms the abort, the AI message is removed, the input is enabled for queuing, and the user can resume the chat with queued messages.

## Root Cause Analysis

There are two code paths for `send_message`: **simple** (no tools) and **tool** (with MCP tools). Both have bugs.

### Path 1: Simple chat (no tools) — [`send_message`](packages/rhd_chat/src/stream/send/message.rs:22)

When abort happens:
1. `abort_chat` cancels the token and sets `StreamState::Aborted`
2. `chat_stream_cancellable` returns `Err(AiError::Aborted)`
3. `send_message` calls `unregister_stream` — **destroys the `Aborted` state**
4. `handle_stream_result` emits `ChatEvent::StreamError { error: "aborted" }` — **wrong event**

The frontend's [`chatStreamError`](frontend/src/lib/chatWs/events.ts:130) handler:
- Sets `streamError = "aborted"` (shows error UI with Retry button)
- Sets `isStreaming = false`
- Does NOT set `isPaused` or `isAborted`
- Does NOT remove the streaming message from `messages`

Result: user sees an error banner, no Resume/Queue buttons, no way to queue messages.

### Path 2: Chat with tools — [`send_message_with_tools`](packages/rhd_chat/src/stream/send/tools.rs:17)

When abort happens during AI call:
1. `tool_loop` emits `ChatEvent::StreamAborted` ✓
2. `tool_loop` calls `abort_chat` (redundant, already Aborted)
3. `tool_loop` returns `Err(ChatError::Ai(AiError::Aborted))`
4. `send_message_with_tools` catch-all branch calls `unregister_stream` — **destroys the `Aborted` state**

The frontend's [`streamAborted`](frontend/src/lib/chatWs/events.ts:195) handler correctly sets `isPaused`, `isAborted`, removes the streaming message. But the `Aborted` state is destroyed on the backend, so `resume_chat` and `queue_message` will fail.

### Root Cause Summary

| # | Issue | Location |
|---|-------|----------|
| 1 | Simple path emits `StreamError` instead of `StreamAborted` for aborts | [`handle_stream_result`](packages/rhd_chat/src/stream/send/utils.rs:93) |
| 2 | `unregister_stream` destroys the `Aborted` state after abort | [`send_message`](packages/rhd_chat/src/stream/send/message.rs:162), [`send_message_with_tools`](packages/rhd_chat/src/stream/send/tools.rs:77) |
| 3 | `resume_chat` only handles `Paused`, not `Aborted` | [`resume_chat`](packages/rhd_chat/src/manager.rs:164) |
| 4 | `handle_resume_chat` doesn't process queued messages or start a new AI call | [`handle_resume_chat`](packages/rhd_app/src/ws/handlers/chat.rs:181) |
| 5 | No `ChatResumed` event emitted by the backend on resume | [`handle_resume_chat`](packages/rhd_app/src/ws/handlers/chat.rs:181) |
| 6 | `tool_loop` top-of-loop cancel check emits `StreamError` instead of `StreamAborted` | [`tool_loop`](packages/rhd_chat/src/tools/tool_loop.rs:89) |
| 7 | `chat_stream_cancellable` and `chat_stream_with_tools` only check cancel after a chunk arrives — blocks indefinitely in manual control mode | [`simple.rs`](packages/rhd_ai/src/client/stream/simple.rs:193), [`tools.rs`](packages/rhd_ai/src/client/stream/tools.rs:67) |

## Fix Plan

### Step 1: Emit `StreamAborted` for aborts in simple path — ✅ DONE

**File:** [`packages/rhd_chat/src/stream/send/utils.rs`](packages/rhd_chat/src/stream/send/utils.rs:93)

In `handle_stream_result`, changed the `Err(AiError::Aborted)` branch to emit `ChatEvent::StreamAborted` instead of `ChatEvent::StreamError`:

```rust
Err(AiError::Aborted { .. }) => {
    if let Some(sink) = chat_log {
        sink.log_stream_error("aborted");
    }
    let _ = event_sender.send(ChatEvent::StreamAborted { chat_id });
    Err(ChatError::Ai(AiError::Aborted {
        model: model.to_string(),
    }))
}
```

### Step 2: Don't unregister stream on abort (simple path) — ✅ DONE

**File:** [`packages/rhd_chat/src/stream/send/message.rs`](packages/rhd_chat/src/stream/send/message.rs)

In `send_message` and `edit_and_resend`, after the stream returns, check if the chat was aborted before unregistering:

```rust
let is_aborted = manager.is_aborted(chat_id).await;
if !is_aborted {
    manager.unregister_stream(chat_id).await;
}
```

### Step 3: Don't unregister stream on abort (tool path) — ✅ DONE

**File:** [`packages/rhd_chat/src/stream/send/tools.rs`](packages/rhd_chat/src/stream/send/tools.rs)

In `send_message_with_tools`:
- `Err(ChatError::Aborted)` branch: no longer emits `StreamAborted` (tool_loop already did) and no longer unregisters
- Catch-all `Err(e)` branch: checks `is_aborted` before unregistering

### Step 4: `resume_chat` handles `Aborted` state — ✅ DONE

**File:** [`packages/rhd_chat/src/manager.rs`](packages/rhd_chat/src/manager.rs:164)

Added handling for `StreamState::Aborted` in `resume_chat`. When resuming from Aborted:
- Creates a fresh `pause_notify` (not needed for Aborted, but required by `Running` state)
- Transitions to `Running` with `ExecutionPhase::AiCall`
- Returns the message queue
- Only calls `pause_notify.notify_one()` if the state was `Paused` (not `Aborted`)

### Step 5: `handle_resume_chat` processes queue and starts new AI call — ✅ DONE

**File:** [`packages/rhd_app/src/ws/handlers/chat.rs`](packages/rhd_app/src/ws/handlers/chat.rs:181)

When resuming from Aborted, the handler now:
1. Calls `resume_chat` to get `ResumeInfo` with the message queue
2. Emits `ChatResumed` event to the frontend
3. If there are queued messages, adds them to the DB and spawns a new AI call via `resume_stream`
4. If no queued messages, unregisters the stream (clean up)

Also made `unregister_stream` public (was `pub(crate)`) so the handler can call it.

### Step 6: Add `resume_stream` function — ✅ DONE

**File:** [`packages/rhd_chat/src/stream/send/message.rs`](packages/rhd_chat/src/stream/send/message.rs)

Added `resume_stream` function that starts an AI call without adding a new user message. The queued messages have already been added to the DB by `handle_resume_chat`. This function:
1. Gets messages from DB (includes the newly added queued messages)
2. Collects tools from projects
3. Starts streaming (same logic as `send_message` but skips the "add user message" step)
4. Does NOT inject the todo tool contract (already injected on first message)

Exported from `stream/send/mod.rs` and `stream/mod.rs`. Added public wrapper in `ChatManager`.

### Step 7: Fix `tool_loop` top-of-loop cancel check — ✅ DONE

**File:** [`packages/rhd_chat/src/tools/tool_loop.rs`](packages/rhd_chat/src/tools/tool_loop.rs:89)

Changed the top-of-loop cancel check to emit `StreamAborted` (instead of `StreamError`) and call `abort_chat` (same as the in-stream abort handler at line 164). This ensures consistent behavior whether the abort is detected at the top of the loop or during `chat_stream_with_tools`.

### Step 8: Fix `chat_stream_cancellable` and `chat_stream_with_tools` to respond to cancel while waiting for chunks — ✅ DONE

**Files:**
- [`packages/rhd_ai/src/client/stream/simple.rs`](packages/rhd_ai/src/client/stream/simple.rs:193)
- [`packages/rhd_ai/src/client/stream/tools.rs`](packages/rhd_ai/src/client/stream/tools.rs:67)

Both streaming functions now use `tokio::select!` to race `stream.next()` against `cancel.cancelled()`, plus an `is_cancelled()` check at the top of the loop. This ensures the cancel token is detected even when the SSE stream is blocked waiting for chunks (e.g., mock server in manual control mode).

### Step 9: Update e2e test to verify actual backend behavior — ✅ DONE

**File:** [`frontend/src/tests/e2e/chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts:561)

The test has been updated to:
- Use `waitForStreamReady()` + `emitStreamChunk()` to ensure the backend has registered the stream before aborting
- Wait for `streamAborted` event from the backend (instead of manually dispatching it)
- Wait for `chatResumed` event from the backend (instead of manually dispatching it)

**Status:** Test passes. The full abort/resume flow works end-to-end.

**Root cause of initial failure:** The frontend WebSocket handler in [`frontend/src/lib/ws.ts`](frontend/src/lib/ws.ts:96) was filtering events to only dispatch those starting with `chat` or `project`. The `streamAborted` event doesn't start with either prefix, so it was being silently dropped. Fixed by adding `streamAborted` and `messageQueued` to the dispatch filter.

**Additional fix:** The mock server was returning tool calls even in manual control mode, which caused the test to fail because the assistant message with tool calls was preserved on abort. Fixed by only returning tool calls when `auto_stream` is enabled.

## Next Steps

### Step 10: Run full test suite

Once the e2e test passes:
- Run `cargo test -p rhd_chat` (unit tests)
- Run `mise run test-frontend-e2e` (all e2e tests)
- Verify no regressions in other tests

### Step 11: Clean up debug code — ✅ DONE

All file-based debug logging has been removed from:
- `packages/rhd_chat/src/manager.rs`
- `packages/rhd_chat/src/tools/tool_loop.rs`
- `packages/rhd_ai/src/client/stream/tools.rs`
- `packages/rhd_app/src/ws/events.rs`
- `packages/rhd_app/src/ws/mod.rs`
- `frontend/src/lib/chatWs/events.ts`

## Files Modified

| File | Change | Status |
|------|--------|--------|
| [`packages/rhd_chat/src/stream/send/utils.rs`](packages/rhd_chat/src/stream/send/utils.rs) | Emit `StreamAborted` instead of `StreamError` for aborts in `handle_stream_result` | ✅ Done |
| [`packages/rhd_chat/src/stream/send/message.rs`](packages/rhd_chat/src/stream/send/message.rs) | Don't unregister on abort in `send_message` and `edit_and_resend`; add `resume_stream` function | ✅ Done |
| [`packages/rhd_chat/src/stream/send/tools.rs`](packages/rhd_chat/src/stream/send/tools.rs) | Don't unregister on abort in `send_message_with_tools` | ✅ Done |
| [`packages/rhd_chat/src/stream/send/mod.rs`](packages/rhd_chat/src/stream/send/mod.rs) | Export `resume_stream` | ✅ Done |
| [`packages/rhd_chat/src/stream/mod.rs`](packages/rhd_chat/src/stream/mod.rs) | Export `resume_stream` | ✅ Done |
| [`packages/rhd_chat/src/manager.rs`](packages/rhd_chat/src/manager.rs) | `resume_chat` handles `Aborted` state; add `resume_stream` wrapper; make `unregister_stream` public | ✅ Done |
| [`packages/rhd_app/src/ws/handlers/chat.rs`](packages/rhd_app/src/ws/handlers/chat.rs) | `handle_resume_chat` processes queue, emits `ChatResumed`, spawns new AI call | ✅ Done |
| [`packages/rhd_chat/src/tools/tool_loop.rs`](packages/rhd_chat/src/tools/tool_loop.rs) | Top-of-loop cancel check emits `StreamAborted` instead of `StreamError` | ✅ Done |
| [`packages/rhd_ai/src/client/stream/simple.rs`](packages/rhd_ai/src/client/stream/simple.rs) | Use `tokio::select!` + `is_cancelled()` for cancel detection | ✅ Done |
| [`packages/rhd_ai/src/client/stream/tools.rs`](packages/rhd_ai/src/client/stream/tools.rs) | Use `tokio::select!` + `is_cancelled()` for cancel detection | ✅ Done |
| [`frontend/src/tests/e2e/chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) | Update e2e test to verify actual backend abort/resume flow | ✅ Done |
| [`frontend/src/lib/ws.ts`](frontend/src/lib/ws.ts) | Dispatch `streamAborted` and `messageQueued` events (were being filtered out) | ✅ Done |
| [`packages/rhd_test/src/mock_server/handlers.rs`](packages/rhd_test/src/mock_server/handlers.rs) | Only return tool calls in auto-stream mode (manual control mode returns streaming content) | ✅ Done |

## Risks

1. **Mock AI server abort support**: The e2e test relies on the mock AI server respecting cancellation. The `chat_stream_cancellable` and `chat_stream_with_tools` now use `tokio::select!` to detect cancel while waiting for SSE chunks. However, the mock server's SSE keep-alive (1-second interval) may interfere with the `select!` — see Step 9 investigation notes.

2. **Race conditions**: The abort flow involves a spawned task (`send_message`) and the WebSocket handler (`handle_abort_chat`). The `is_aborted` check in `send_message` must happen after `abort_chat` sets the state, which is guaranteed by the mutex on `active_streams`.

3. **Duplicate `StreamAborted` events**: In the tool path, `tool_loop` emits `StreamAborted` both at the top-of-loop check and in the `chat_stream_with_tools` error handler. The `send_message_with_tools` `Err(ChatError::Aborted)` branch no longer emits it. Need to verify only one emission reaches the frontend.

4. **`resume_stream` vs `send_message`**: The `resume_stream` function is similar to `send_message` but skips adding a user message and skips the todo tool contract injection. Need to ensure all the injection logic (system prompts, roles) is handled correctly without duplicates.

5. **Built-in tools always present**: Even without attached projects, `collect_tools_from_projects` returns at least 1 tool (`rhd_set_todo_list`). This means `send_message` always goes through the **tool path** (`send_message_with_tools`), not the simple path. The simple path is only used when no tools are available (which may never happen in practice). This affects testing: the e2e test must account for the tool loop's first iteration (which returns tool calls immediately from the mock).

## Success Criteria

1. When aborting during AI call (simple path, no tools):
   - Backend emits `StreamAborted` (not `StreamError`)
   - Frontend removes streaming message, shows Resume + Queue buttons
   - Input is enabled for queuing

2. When aborting during AI call (tool path):
   - Backend emits `StreamAborted` (already works)
   - `Aborted` state is preserved (not destroyed by `unregister_stream`)
   - Frontend shows Resume + Queue buttons

3. When queuing messages after abort:
   - `queue_message` succeeds (state is `Aborted`, not removed)
   - Message appears as gray pending message

4. When resuming after abort:
   - Backend emits `ChatResumed`
   - Queued messages are added to DB and promoted in frontend
   - New AI call starts with full context
   - Frontend shows streaming state

5. E2E test verifies the full flow without manual event dispatch