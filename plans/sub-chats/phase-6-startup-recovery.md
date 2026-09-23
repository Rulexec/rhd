# Phase 6: Startup Recovery — Re-drive Unfinished Tool Calls

> Parent plan: [`rhd-plugin-sub-chat-grand-plan.md`](../rhd-plugin-sub-chat-grand-plan.md) (AD-7)

## Overview

Prove and implement the zero-persistence promise: after any plugin/server restart, every unfinished unit of sub-chat work — a sync spawn left unactivated, a paused subchat with a partial queue, a live await, a status call that died before answering, an async call that died before its notice was sent — is discovered from chat history alone and re-driven to the correct end state, exactly once. The scan runs once at startup after the monitors and the watcher hook are installed, reusing the existing primitives verbatim: `converge_spawn` (Phase 3), `handle_status` / `handle_await` (Phase 5), `Watcher::register_or_complete` (Phase 4).

**Scope in:** `recovery.rs`; `AnswerGuards` extension with a per-`tool_call_id` **claim set** (processing serialization); small `plugin.rs` wiring; tests.
**Scope out:** changes to spawn/completion/watcher semantics; custom-event ack recovery (existing `getPendingAcks` drain covers it).

**Dependencies:** Phases 3, 4, 5 (all reused unchanged). Runs while the plugin is live — safe by the guards introduced here.

## Wire Contract (fixed)

| Item | Value |
|---|---|
| Discovery source | per chat: `getMessages(chatId, withUnresolvedToolCalls = true)` (server computes: assistant messages with declared calls lacking a `tool` answer anywhere in stored history) |
| Subchat lookup | tag scan: a chat whose tags contain `sub_chat:call:<toolCallId>` |
| Re-drive | identical to live path — `converge_spawn` is idempotent from any crash point (create-if-absent / prefix-complete a `paused` queue / unpause-only-when-complete) |
| Late async answer | if an async spawn's call is still unanswered (crash before the notice), answer `async_notice(B)` now — correct and harmless late |
| Dedup | `has_tool_result` (persisted) + in-memory claim set (this phase) make the scan safe against concurrent live handling |

## Files to Create/Modify

### 1. `plugins/rhd_plugin_sub_chat/src/reply.rs` (modify)

Add processing serialization (answer dedup alone can't stop two **spawns**):

```rust
pub struct AnswerGuards {
    answered: tokio::sync::Mutex<HashSet<String>>,
    /// tool_call_ids currently being processed by a live handler or the scan.
    processing: tokio::sync::Mutex<HashSet<String>>,
}

impl AnswerGuards {
    /// Try to own the processing of `tool_call_id`. false → someone else is on it; skip.
    pub async fn claim(&self, tool_call_id: &str) -> bool;
    pub async fn release(&self, tool_call_id: &str);
    // answer_once unchanged.
}
```

**Every entry point claims**: `handle_spawn`, `handle_status`, `handle_await` bodies wrap their whole processing in `claim → … → release` (RAII wrapper struct recommended to survive early returns), then keep the `has_tool_result` check as the persisted second line of defense. When a claim fails, the entry point simply returns — the owner will answer or register.

### 2. `plugins/rhd_plugin_sub_chat/src/recovery.rs` (new)

```rust
use rhd_chat_api::{GetMessagesParams, ListChatsParams};
use rhd_chat_client::{ChatClient, ChatMonitor};

#[derive(Debug, Default)]
pub struct RecoveryReport {
    pub scanned_calls: usize,
    pub respawned: usize,       // B was absent → full converge
    pub reconciled_paused: usize, // B existed paused → queue completed + activated
    pub rewatched: usize,       // live sync/await waiters re-registered
    pub answered: usize,        // immediate answers (status, await-completed, late async, errors)
    pub skipped_answered: usize,
}

/// Once at startup, reconcile every unfinished rhd_sub_chat* tool call.
pub async fn run(
    client: Arc<ChatClient>,
    chat_monitor: &ChatMonitor,
    ctx: handler::HandlerCtx,          // carries watcher + guards
) -> Result<RecoveryReport, RecoveryError> {
    // 1. Index: list_chats(ListChatsParams { tags: vec![] }) → collect all chats;
    //    map tool_call_id → chat via tags::parse_call_tag over each summary's tags.
    //    (Duplicate link tags — pathological double-create — keep the lowest chat id,
    //     log warn.)
    // 2. For chat_id in chat_monitor.get_chat_ids():
    //      msgs = get_messages(GetMessagesParams { chat_id, with_unresolved_tool_calls: true, ..default })
    //      for m in msgs, for tc in m.tool_calls where tc.function.name in OUR_THREE:
    //          guards.claim(tc.id) else continue;                     // live path owns it
    //          has_tool_result(chat_id, tc.id) → skip_answered;       // persisted guard
    //          dispatch by name (see below); guards.release(tc.id).
    // 3. tracing::info!(report fields, "sub_chat recovery finished")
}
```

**Dispatch per unresolved call (all shared with the live path):**

- `rhd_sub_chat` → `spawn::parse_spawn_request(args)`:
  - `Err(Validation)` → `answer_once` with the validation error text (same as live).
  - `Ok(request)` → `converge_spawn(&SpawnPlan { caller_chat_id: chat_id, tool_call_id: tc.id, request })`:
    - `Err(Unreconcilable)` → error answer, B stays `paused` (AD-13 — never guess).
    - `Err(Server)` → error answer per live formatting.
    - `Ok(B)` → `request.spawn_async` → `answer_once(async_notice(B))` (counts `answered`; the late-notice case is indistinguishable from "crash before notice" and self-heals); sync → `watcher.register_or_complete(B, Waiter { parent_chat_id: chat_id, tool_call_id: tc.id })`.
    - Report classification: index had no B → `respawned`; B found still `paused` → `reconciled_paused`; B found active + sync → `rewatched`; B active + async → `answered`.
- `rhd_sub_chat_status` → parse `chatId` from args → `handler::handle_status(...)` (re-validates direct-child fresh — the world may have changed while the plugin was down).
- `rhd_sub_chat_await` → parse `chatId` → `handler::handle_await(...)` (answers if complete now, else re-registers).

### 3. `plugins/rhd_plugin_sub_chat/src/plugin.rs` (modify)

Insert the run between hook installation and the keep-alive loop:

```rust
// watcher::install(...) already done
match recovery::run(Arc::clone(&client), &chat_monitor, ctx.clone()).await {
    Ok(report) => tracing::info!(?report, "startup recovery complete"),
    Err(e) => tracing::error!(error = %e, "startup recovery failed — some subchat calls may stay parked"),
}
```

Recovery failure must **not** abort the plugin: parked calls remain parked; the next restart re-drives (everything is idempotent). Startup ordering rationale: the scan runs **after** `subscribe_to_all_chats` + `watcher::install` so any state changes it itself causes (unpausing, queueing) flow through the live hook and a completion landing mid-scan is caught by `register_or_complete`'s evaluate-after-insert.

### 4. `plugins/rhd_plugin_sub_chat/src/lib.rs` (modify)

Add `pub mod recovery;`.

## Tests

**Unit (inline in `recovery.rs`):**
- Index build: summaries with `sub_chat:call:a`/`sub_chat:call:b` → map; malformed `sub_chat:call_` ignored; duplicate link tags → lowest id + no panic.
- Report accounting is driven from the pure classify helper if extracted; otherwise covered integration-side.

**Integration — `plugins/rhd_plugin_sub_chat/tests/recovery_test.rs`** (chat server + this plugin; no ai_completions/mock, no real process kill — construct the crash states directly):
1. **Paused + partial queue:** create B with tags `[parent:A, root:A, sub_chat:call:tcX, paused]`, queue only the first of two planned messages; put an assistant message in A carrying the `rhd_sub_chat` `toolCallId: "tcX"` (create via `add_message` + `update_message` with the `toolCalls` JSON — verify the server accepts `updateMessage` from a plain plugin connection while writing this test; if it refuses, this specific scenario moves to the Phase 7 mock harness). Start the plugin → assert recovery enqueued message #2 in order, removed `paused`, left A's call unanswered (sync plan), `reconciled_paused == 1`.
2. **B absent:** unresolved sync call in A, no B → after startup B exists activated with the full queue, waiters registered (`watched_ids` contains B); append finished assistant to B → A answered verbatim.
3. **Late async:** async call unresolved + B already active & completed → A receives the exact `async_notice(B)` once.
4. **Already answered:** B complete + A already has the tool answer → untouched (`skipped_answered == 1`, no duplicate message).
5. **Await across restart:** unresolved `rhd_sub_chat_await` call on completed B → answered immediately with final content.
6. **Tamper refusal:** paused B whose queue mismatches the plan (foreign content) → error answer in A, B **still paused**.
7. **Idempotent double-run:** call `recovery::run` twice back-to-back → zero additional chats/messages/answers.

## Implementation Notes

1. **The claim set is the only new concurrency primitive** — keep it strictly "claim outermost, release last"; a nested `converge` must not re-claim its own id (it receives the plan, not the call).
2. **Why `list_chats` once, not per call:** recovery touches every chat anyway; the single unfiltered call is cheaper and gives the duplicate-link-tag diagnostic.
3. **`with_unresolved_tool_calls` returns messages whose calls lack answers in stored history** — exactly our definition of unfinished; no client-side re-scan needed beyond name filtering. Our own new answers during the scan can't desync the loop because we re-read per chat before iterating calls.
4. **Deleted callers:** `getMessages` for a vanished chat errors → log + skip that chat (waiters for it can never be answered; the scan just re-discovers the emptiness next restart — harmless).
5. **Scale note (informational):** scan is O(chats × messages-with-calls) once at startup — fine at this project's scale; if startup ever lags, parallelize per chat with `JOIN_SET`-style bounded fan-out, not a timer.

## Dependencies

- **Requires:** Phase 3 (`converge_spawn` — reused verbatim; that reuse IS the design validation), Phase 4 (watcher API), Phase 5 (`handle_status`/`handle_await` as the frozen seam).
- **Blocks:** Phase 7 (full-restart e2e story). Parallel with Phase 8 drafting.
