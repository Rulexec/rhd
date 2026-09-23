# Phase 3: Spawn Engine and Async Answer Path

> Parent plan: [`rhd-plugin-sub-chat-grand-plan.md`](../rhd-plugin-sub-chat-grand-plan.md) (AD-3, AD-4, AD-6, AD-13)

## Overview

Implement the `rhd_sub_chat` tool-call handling and the two-phase activation spawn protocol: **validate → look up/create the subchat with the full atomic tag set (incl. `paused`) → reconcile + enqueue starting messages → remove `paused`**. Async mode (`async: true`) answers the tool call immediately after activation with the fixed background notice. Sync mode spawns identically but leaves the call unanswered through an explicit seam that Phase 4's watcher will fill. The converge logic is factored as a reusable idempotent primitive — Phase 6 recovery calls the very same function.

**Scope in:** `reply.rs`, `spawn.rs`, `handler.rs` (rhd_sub_chat arm only), `on_tool_call` subscription, async answer, unit + integration tests.
**Scope out:** completion watching, sync answers (Phase 4), `rhd_sub_chat_status` / `rhd_sub_chat_await` handling (Phase 5), recovery run (Phase 6 — but converge must already be restart-safe by design here).

**Dependencies:** Phase 2 (scaffold, `tags.rs`, `templates.rs`). Phase 1 must be deployed for the two-phase activation guarantee to actually hold (a paused chat must not trigger).

## Wire Contract (fixed)

| Item | Value |
|---|---|
| Spawn tag set on B (atomic in `createChat`) | `[parent_tag(A), root_tag(R), call_tag(toolCallId), <user tags...>, PAUSED_TAG]` |
| Root `R` | `tags::root_of(A.tags)` if present, else `A` itself (and A gets `root:A` added) |
| Async notice | `Chat started in background with id {chat_id}, use tools rhd_sub_chat_status or rhd_sub_chat_await on it` |
| Sync seam (this phase) | `handler` logs `tracing::warn!("sync rhd_sub_chat parked for subchat {id} — watcher lands in Phase 4")` and leaves the call unanswered |
| Validation failure answer | single-line string starting `rhd_sub_chat error: ` |
| Mid-spawn server failure answer | `rhd_sub_chat error: failed to prepare subchat {id or new}: <detail> (subchat left paused for recovery)` |

## Files to Create/Modify

### 1. `plugins/rhd_plugin_sub_chat/src/reply.rs` (new)

Shared answering utilities (all later phases answer through here):

```rust
//! Idempotent tool-call answering helpers.

use rhd_chat_api::{AddMessageParams, GetChatParams};
use rhd_chat_client::{ChatClient, ClientError};

/// True if a `tool`-role message answering `tool_call_id` already exists in `chat_id`
/// (todo_list's has_tool_result pattern; guard against duplicate answers).
pub async fn has_tool_result(client: &ChatClient, chat_id: i64, tool_call_id: &str) -> Result<bool, ClientError> {
    let chat = client.get_chat(GetChatParams { chat_id, if_version_higher_than: None }).await?;
    Ok(chat.messages.iter().any(|m| m.tool_call_id.as_deref() == Some(tool_call_id)))
}

/// Post a `tool`-role answer for `tool_call_id` in `chat_id`.
pub async fn answer_tool_call(
    client: &ChatClient,
    chat_id: i64,
    tool_call_id: &str,
    content: String,
) -> Result<(), ClientError> {
    client.add_message(AddMessageParams {
        chat_id,
        role: "tool".to_string(),
        content,
        tool_call_id: Some(tool_call_id.to_string()),
        reasoning_content: None,
        tags: vec![],
        is_finished: true,
        is_streaming: false,
    }).await
}
```

Phase 4 will add `AnswerGuards` here (in-memory in-flight set) — design the module so that is additive.

### 2. `plugins/rhd_plugin_sub_chat/src/spawn.rs` (new)

**Types** (serde derives; `camelCase` wire, `#[serde(rename = "async")]` for the flag):

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StartingMessage { pub role: String, pub content: String }

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SpawnRequest {
    pub messages: Vec<StartingMessage>,
    #[serde(default)] pub tags: Vec<String>,
    #[serde(default, rename = "async")] pub spawn_async: bool,
}

/// Immutable description of one rhd_sub_chat call, replayable at any time (live or recovery).
#[derive(Debug, Clone)]
pub struct SpawnPlan {
    pub caller_chat_id: i64,
    pub tool_call_id: String,
    pub request: SpawnRequest,
}

#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    #[error("validation: {0}")]           Validation(String),      // → error-answer, no chat created
    #[error("cannot reconcile paused subchat {sub_chat_id}")]     Unreconcilable { sub_chat_id: i64 },
    #[error("server: {0}")]  Server(String),                       // any client-call failure, ctx included
}
```

**Pure helpers** (unit-test surface):

```rust
/// Parse & validate the JSON arguments string (roles ∈ {system,user}, non-empty messages,
/// content strings, tags via tags::validate_user_tags). Errors → SpawnError::Validation.
pub fn parse_spawn_request(arguments: &str) -> Result<SpawnRequest, SpawnError>;

/// `sub · ` + first 40 chars of the first user message (fallback: first message; fallback: "subchat"),
/// `…` appended when truncated.
pub fn generated_title(messages: &[StartingMessage]) -> String;

/// The exact async notice (Wire Contract).
pub fn async_notice(chat_id: i64) -> String;

/// Indices of `expected` messages missing from the observed queue.
/// Queue items must equal expected[i] (role+content) in order; any queue item beyond
/// expected length or any content mismatch → Err(()) (human tampering; refuse to guess).
pub fn plan_missing_suffix(queue: &[rhd_chat_api::Message], expected: &[StartingMessage]) -> Result<Vec<usize>, ()>;
```

**The converge primitive** — idempotent "drive this plan to *activated*" function (called by both the live handler and Phase 6 recovery):

```rust
/// Converge the world to "subchat for `plan` exists, has all starting messages queued,
/// and is activated (not paused)". Returns the subchat id. Safe to call any number of
/// times from any crash point. NEVER unpauses before the queue is complete.
pub async fn converge_spawn(client: &ChatClient, plan: &SpawnPlan) -> Result<i64, SpawnError> {
    // 1. Ensure caller lineage: get_chat(caller) → root R = tags::root_of(tags)
    //    .unwrap_or(caller); if caller has no root tag → update_chat(add_tags=[root_tag(caller)]).
    // 2. Find existing: list_chats(ListChatsParams { tags: vec![call_tag(&plan.tool_call_id)] })
    //    → 0 or >1 matches: create; exactly 1: reuse (log at info).
    //    (Create: create_chat(CreateChatParams { title: generated_title(...),
    //      tags: Wire-Contract tag set })) — all tags, incl. PAUSED_TAG, in ONE call
    //    → structurally guarantees "tags before queue".
    // 3. If B still has PAUSED_TAG: get_queue_messages(B) → plan_missing_suffix(queue, msgs)
    //    (Err → SpawnError::Unreconcilable); add_queue_message each missing msg
    //    (role, content; no tags).
    // 4. If B still has PAUSED_TAG: update_chat(B, remove_tags=[PAUSED_TAG]) → activation point.
    // Every client call: map_err(|e| SpawnError::Server(format!("step <n>: {e}"))).
}
```

**Implementation notes for converge:**
- Step 2 create races: two converge runs (live event + recovery) could both create. Tolerable in Phase 3 (recovery doesn't exist yet); Phase 6 will serialize via its single startup pass + answer guards. Document with a `// NOTE(Phase 6):` comment.
- Step 3 must re-check the pause tag from the same fetched state used in step 2 (get_chat for B's tags), not from a stale list summary — tags are present in `ChatSummary` from `list_chats`, that's sufficient here.
- If B was found but `plan_missing_suffix` errors, B is left `paused` untouched (AD-13).

### 3. `plugins/rhd_plugin_sub_chat/src/handler.rs` (new)

```rust
use rhd_chat_api::AssistantMessageWithToolCallsData;

#[derive(Clone)]
pub struct HandlerCtx {
    pub client: std::sync::Arc<ChatClient>,
    // Phase 4 adds: pub watcher: Arc<Watcher>,
}

/// One rhd_sub_chat call inside an assistant message. Returns Ok(()) — errors are
/// answered, not propagated (a failed answer is only logged).
pub async fn handle_spawn(ctx: HandlerCtx, caller_chat_id: i64, tool_call: rhd_chat_api::ToolCall) {
    // a. reply::has_tool_result guard → skip if answered.
    // b. parse_spawn_request(arguments): Err(Validation(msg)) → answer "rhd_sub_chat error: {msg}".
    //    Err(other) → log + answer generic.  (Server-side args are strings from the provider;
    //    treat unparseable JSON as Validation.)
    // c. plan = SpawnPlan { caller_chat_id, tool_call_id: tool_call.id, request }.
    // d. converge_spawn → Ok(sub_chat_id) / Err(e):
    //    - Validation already handled; on Unreconcilable/Server: answer
    //      "rhd_sub_chat error: ... (subchat left paused for recovery)" including id when known,
    //      and return.
    // e. if request.spawn_async → answer_tool_call(caller_chat_id, id, async_notice(sub_chat_id)).
    //    else → Wire-Contract sync seam log (Phase 4 replaces this arm with watcher registration).
}

/// Entry for the on_tool_call subscription (Phase 5 extends the name list + dispatch).
pub async fn handle_tool_call_event(ctx: HandlerCtx, event: AssistantMessageWithToolCallsData) {
    for tool_call in &event.message.tool_calls {
        match tool_call.function.name.as_str() {
            "rhd_sub_chat" => handle_spawn(ctx.clone(), event.chat_id, tool_call.clone()).await,
            _ => {} // status/await arrive in Phase 5
        }
    }
}
```

### 4. `plugins/rhd_plugin_sub_chat/src/plugin.rs` (modify)

After the tool-registration wiring (Phase 2), add the subscription — subscribe **once with all three tool names** even though only `rhd_sub_chat` is dispatched (avoids recreating the subscription in Phase 5; status/await events simply fall through):

```rust
let ctx = HandlerCtx { client: Arc::clone(&client) };
client.on_tool_call(0, vec![
    "rhd_sub_chat".to_string(),
    "rhd_sub_chat_status".to_string(),
    "rhd_sub_chat_await".to_string(),
], move |event| {
    let ctx = ctx.clone();
    async move { handler::handle_tool_call_event(ctx, event).await }
});
```

(`tokio::spawn` discipline: `on_tool_call` callbacks already run in spawned tasks by the client dispatcher — verify against todo_list usage while implementing; if not, wrap.)

### 5. `plugins/rhd_plugin_sub_chat/src/lib.rs` (modify)

Add `pub mod handler; pub mod reply; pub mod spawn;`.

## Tests

**Unit (inline):**
- `parse_spawn_request`: valid sync/async JSON; `messages: []` rejected; role `assistant`/`tool` rejected; non-string content rejected; unknown top-level key tolerated (`serde` default) but missing `messages` rejected; reserved user tag rejected via validate_user_tags path; `async` absent → false, `true` → true.
- `generated_title`: user-first selection, 40-char truncation + `…`, system-only fallback, empty → `sub · subchat`.
- `async_notice`: exact string lock-in test (golden — other phases and e2e assert against this).
- `plan_missing_suffix`: empty queue → all; exact full prefix → suffix list; already complete → `[]`; mismatched content → Err; extra tampering queue item → Err.

**Integration — `plugins/rhd_plugin_sub_chat/tests/spawn_async_test.rs`** (chat server + this plugin only; no ai_completions, no mock AI):
1. Test client connects, registers as plugin (`RegisterPluginParams`), creates chat A, seeds it via `add_message(role="assistant", content=..., )` — assistant-with-tool_calls cannot be created via `addMessage` (no `tool_calls` param); use the same trick as the todo_list/choice integration tests if present, otherwise register the `rhd_sub_chat` tool and drive it by directly calling `spawn::converge_spawn` is not allowed from tests of binaries — instead: assert through `handler::handle_spawn` exported API in an integration test linking the plugin lib crate (`rhd_plugin_sub_chat` is a lib+bin, tests import it freely) with a manually-built `rhd_chat_api::ToolCall`.
2. Assertions after `handle_spawn` with `async: true`: exactly one chat exists with `sub_chat:call:<id>`; its tags include `parent:<A>` and `root:<A>`; A has `root:<A>`; queue holds the starting messages in order; `paused` removed; A gained a `tool` message equal to `async_notice(B)` with the right `tool_call_id`.
3. Idempotency: run `handle_spawn` again with same args → no second chat (link tag found), no duplicate tool answer (has_tool_result guard), queue unchanged.
4. Validation error: bad role → error answer in A, **no chat created**.

## Implementation Notes

1. **Why two-phase activation:** without `paused`, ai_completions could trigger on the first queued message while the plugin is still enqueueing the rest (it drains the whole queue at trigger time — race) and no crash point would be distinguishable from a "real" partial task. With `paused` every crash state is *explicit* and re-drivable (AD-3).
2. **`converge_spawn` is the recovery kernel** — keep it free of answering/handler concerns; it only manipulates chats. The caller decides who gets what answer.
3. **Title collision:** several subchats may share a generated title; the link tag + `parent:` are the identity, titles are cosmetic.
4. **Ordering:** `add_queue_message` appends by internal position — enqueue strictly in `messages` order, sequentially (do not parallelize; the provider history must read system→user in seed order).
5. **Do not answer on `SpawnError::Server` in step-2-create failure when no B exists** — the "subchat left paused" text applies only when an id exists; use the `new` fallback from the Wire Contract.

## Dependencies

- **Requires:** Phase 2 complete (modules, tags, templates); Phase 1 deployed (pause gate) for the activation protocol to be meaningful end-to-end.
- **Blocks:** Phase 4 (watcher consumes the sync seam), Phase 6 (recovery reuses `converge_spawn`, `SpawnPlan`, `parse_spawn_request` unchanged), Phase 7.
