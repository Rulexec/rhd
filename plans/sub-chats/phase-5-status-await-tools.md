# Phase 5: `rhd_sub_chat_status` and `rhd_sub_chat_await` Tools

> Parent plan: [`rhd-plugin-sub-chat-grand-plan.md`](../rhd-plugin-sub-chat-grand-plan.md) (AD-2, AD-5, AD-6)

## Overview

Handle the two remaining tool names: **status** answers a fresh evaluation of the completion predicate (literal `pending` / `completed`), **await** answers the final assistant content when the subchat is already idle or registers a watcher otherwise. Both enforce the **direct-child rule** (AD-5): the target must carry `parent:<callerChatId>`, everything else is a tool error. This makes cyclic awaits impossible by construction (a chat can never be its own parent), and scopes all waiting to explicitly-delegated children. The per-tool handlers are extracted as plain functions so Phase 6 recovery calls the identical code paths.

**Scope in:** `handler.rs` status/await arms, shared state-fetch + validation helper, integration tests.
**Scope out:** predicate/watcher internals (Phase 4, consumed as-is), recovery (Phase 6), any resume/append-to-existing-chat ability (out of scope — `rhd_sub_chat` only spawns new chats).

**Dependencies:** Phase 4 (`completion.rs`, `Watcher::register_or_complete`). Phase 3's subscription already filters these tool names — only the dispatch arms are new.

## Wire Contract (fixed)

| Item | Value |
|---|---|
| Status answer | exactly `pending` or `completed` (no reason suffix — user decision) |
| Await answer | last assistant `content` verbatim (same as sync spawn) |
| Unknown target | `rhd_sub_chat_{status,await} error: chat <id> not found` |
| Not a direct child | `... error: chat <id> is not a direct subchat of this chat` |
| Direct-child rule | target's tags satisfy `tags::is_direct_child(&target_tags, caller_chat_id)` (exact `parent:<caller>` tag) |
| Fresh evaluation | status never consults cached watcher state; always fetches the target chat |
| Timeout | none — await holds until completion (parking the caller's loop is the mechanism) |

## Files to Modify

### 1. `plugins/rhd_plugin_sub_chat/src/handler.rs` (modify)

**Add the shared lookup/validation step:**

```rust
/// Fetch a chat and enforce the direct-child rule. Returns the fresh ChatState.
/// Left-value strings are ready-to-send error answers.
async fn ensure_direct_child(
    client: &ChatClient, caller_chat_id: i64, target_chat_id: i64,
) -> Result<ChatState, String> {
    match client.get_chat(GetChatParams { chat_id: target_chat_id, if_version_higher_than: None }).await {
        Err(e) if is_not_found(&e) => Err(format!("chat {target_chat_id} not found")),
        Err(e) => Err(format!("chat {target_chat_id} lookup failed: {e}")),
        Ok(r) if !tags::is_direct_child(&r.chat.tags, caller_chat_id) =>
            Err(format!("chat {target_chat_id} is not a direct subchat of this chat")),
        Ok(r) => Ok(watcher::chat_state_from_get(&r)),   // pub(crate) — already exists (Phase 4)
    }
}
```

(`is_not_found`: match on `ClientError`/server error text as available; if the server returns a generic error for unknown ids, any fetch error maps to "not found" for simplicity — note it and keep the mapping in one place.)

**Add per-tool functions** (pure dispatch, no `ToolCall` coupling — Phase 6 calls these with values parsed from stored arguments):

```rust
pub async fn handle_status(ctx: &HandlerCtx, tool: &str /*name for error prefix*/,
                           caller_chat_id: i64, tool_call_id: &str, target_chat_id: i64) {
    // guard via ctx.guards (answer_once skips answered calls) at the point of answering:
    // 1. ensure_direct_child → Err(msg) → answer "rhd_sub_chat_status error: {msg}" via answer_once.
    // 2. let text = if completion::is_completed(&state) { COMPLETED } else { PENDING };
    // 3. answer_once(caller_chat_id, tool_call_id, text.to_string()).
}

pub async fn handle_await(ctx: &HandlerCtx, tool: &str,
                          caller_chat_id: i64, tool_call_id: &str, target_chat_id: i64) {
    // 1. ensure_direct_child → Err(msg) → answer "rhd_sub_chat_await error: {msg}".
    // 2. if is_completed → answer_once(final_answer(state).unwrap_or_default()).
    // 3. else ctx.watcher.register_or_complete(target_chat_id,
    //        Waiter { parent_chat_id: caller_chat_id, tool_call_id: tool_call_id.into() }).
}
```

**Extend `handle_tool_call_event` dispatch:**

```rust
match tool_call.function.name.as_str() {
    "rhd_sub_chat" => handle_spawn(/* as-is */).await,
    "rhd_sub_chat_status" | "rhd_sub_chat_await" => {
        let args = parse_target_chat_id(&tool_call.function.arguments); // {"chatId": int}
        match args {
            Ok(id) if name == "rhd_sub_chat_status" => handle_status(&ctx, name, event.chat_id, &tool_call.id, id).await,
            Ok(id) => handle_await(&ctx, name, event.chat_id, &tool_call.id, id).await,
            Err(msg) => answer_once(ctx, event.chat_id, &tool_call.id,
                                    format!("{name} error: invalid arguments: {msg}")).await,
        }
    }
    _ => {}
}
```

Refactor `handle_spawn` to also route all its answers through `ctx.guards.answer_once` (Phase 4 note anticipated this; if not done, do it here). Keep `HandlerCtx` fields: `client`, `watcher`, `guards`.

### 2. No changes to `plugin.rs`, `watcher.rs`, `completion.rs`

The `on_tool_call` subscription (Phase 3) already includes all three names; the watcher handles awaits transparently — an `await` waiter is indistinguishable from a sync-spawn waiter inside `Watcher` (that reuse is the point).

## Tests

**Unit (inline in `handler.rs` where pure):** `parse_target_chat_id`: `{"chatId":5}` → 5; missing key / non-integer / non-object JSON → error strings. `ensure_direct_child` text-mapping kept trivial — cover via integration.

**Integration — `plugins/rhd_plugin_sub_chat/tests/status_await_test.rs`** (chat server + this plugin; spawn subchats via `handle_spawn` like Phase 3/4 tests):
1. **Direct-child rule:** chat B1 `parent:<A>`-tagged (real spawn) and B2 with `parent:<other>` (create directly with tags) — `rhd_sub_chat_status`/`await` calls from A targeting B2 → error answer text as per Wire Contract; targeting B1 → works. Grandchild denial: spawn C from B1 (tags `parent:B1`), status from A on C → error. Self-await (A on A) → error (no `parent:A` on A).
2. **Pending → completed:** after spawn, status → `pending` (queue>0 right after activation in production; here simulate by leaving a queued message); append finished assistant to B → status → `completed`.
3. **Await fast path:** await on an already-completed B → answered immediately without watcher registration (assert `watched_ids()` does not contain B).
4. **Await slow path:** await on pending B → unanswered; append finished assistant → answered verbatim; **two awaits** on the same B from two different tool calls in A → both answered same content (multi-waiter broadcast).
5. **Status freshness:** no cache — toggle B between pending/completed by message edits and re-query; answers flip accordingly.
6. **Error answers resolve the loop:** all error cases produce a `tool` message (the caller's loop must never park on a rejected call).

## Implementation Notes

1. **Why fresh evaluation for status** (user decision Q19): cached watcher state can lag; a status call is cheap (`getChat`) and must reflect "right now". `completed` here means "idle per predicate", not "has ever completed" — if a human queues new work in B, status correctly returns `pending` again.
2. **Await on a completed-then-active B** registers and waits for the *next* idle transition — consistent with (1), no special-casing.
3. **Cycles are structurally impossible:** B awaiting A would require `parent:<B>` on A, which only exists if A was spawned by B — spawn provenance is a DAG via monotone chat creation ids. No chain walk needed (AD-5).
4. **Grandchild UX:** the model is told via the error string what the boundary is; the tool description (Phase 2 JSON) already states "only direct children". If demand arises later, a "walk parent chain" status mode is an easy follow-up — explicitly not now.
5. **Do not gate status/await on who spawned them within the chat** — any unresolved call *shape* is fine; the direct-child tag is the only authorization.

## Dependencies

- **Requires:** Phases 1–4 (all runtime behavior).
- **Blocks:** Phase 6 (recovery calls `handle_status`/`handle_await` directly; their signatures are the frozen seam), Phase 7 (full contract coverage).
