# Phase 4: Completion Predicate, Watcher, and Sync Answer Path

> Parent plan: [`rhd-plugin-sub-chat-grand-plan.md`](../rhd-plugin-sub-chat-grand-plan.md) (AD-1, AD-2, AD-6, AD-7)

## Overview

Close the loop for sync mode: a pure completion predicate over chat state (the single encoding of AD-2), a `Watcher` registry of pending tool-call waiters keyed by subchat id, and a `ChatMonitor` state-change hook that answers **all** waiters with the subchat's final assistant content the moment the predicate first holds. The Phase 3 sync seam is replaced with real watcher registration. Also introduces `AnswerGuards` (in-flight dedup) where the two answer paths (monitor hook, register-or-complete) first converge.

**Scope in:** `completion.rs`, `watcher.rs`, `AnswerGuards` in `reply.rs`, `plugin.rs` hook installation, `handler.rs` sync arm.
**Scope out:** `rhd_sub_chat_status` / `rhd_sub_chat_await` tool handling (Phase 5 — but `Watcher::register_or_complete` is built to be their engine), recovery (Phase 6).

**Dependencies:** Phase 3 (spawn seam, `reply.rs`). Phase 1 only indirectly (real subchats complete via `ai_completions`, but tests here drive completion synthetically). Parallel with Phase 5 only after `Watcher` API freezes (it's the shared surface — do Phase 4 first).

## Wire Contract (fixed)

| Item | Value |
|---|---|
| Completion predicate | last message is `assistant` ∧ `is_finished` ∧ `!is_streaming` ∧ `tool_calls.is_empty()` ∧ `queued_messages_count == 0` ∧ no `ai_completions:running` ∧ no `ai_completions:error` tag |
| Sync answer content | last assistant message `content` **verbatim** (empty string if empty) |
| Waiter | `Waiter { parent_chat_id: i64, tool_call_id: String }` |
| Watcher storage | in-memory `HashMap<sub_chat_id, Vec<Waiter>>` behind `tokio::sync::RwLock`; **never persisted** (recovery in Phase 6 rebuilds) |
| Take-before-answer | waiters are atomically removed from the map before being answered (prevents double-fire from concurrent state-change events); a failed answer is logged and dropped (AD-7: deleted chats) |

## Files to Create/Modify

### 1. `plugins/rhd_plugin_sub_chat/src/completion.rs` (new)

```rust
//! The single source of truth for "the subchat is done" (grand plan AD-2).

use rhd_chat_api::Message;
use rhd_chat_client::ChatState;

pub const PENDING: &str = "pending";
pub const COMPLETED: &str = "completed";

pub fn is_completed(state: &ChatState) -> bool {
    // tags gate
    !has_blocking_tag(&state.tags)
        && state.queued_messages_count == 0
        && match state.messages.last() {
            Some(last) => is_final_assistant(last),
            None => false,
        }
}

/// Assistant, fully written, carrying no tool calls.
fn is_final_assistant(msg: &Message) -> bool {
    msg.role == "assistant" && msg.is_finished && !msg.is_streaming && msg.tool_calls.is_empty()
}

fn has_blocking_tag(tags: &[String]) -> bool {
    tags.iter().any(|t| t == "ai_completions:running" || t == "ai_completions:error")
}

/// Content of the final assistant message when `is_completed` holds.
pub fn final_answer(state: &ChatState) -> Option<&str> {
    state.messages.last().filter(|m| is_final_assistant(m)).map(|m| m.content.as_str())
}
```

**Why "no unresolved tool calls" needs no scan here:** if the *last* message is an assistant message declaring tool calls, no `tool` result can exist *after* it — so `tool_calls.is_empty()` on the last message is exactly equivalent to "last loop resolved" for the completion check (the general `tool_resolution` scan is not needed; noted for reviewers comparing with ai_completions). Keep a doc comment saying so.

### 2. `plugins/rhd_plugin_sub_chat/src/reply.rs` (modify)

Add the in-flight guard set (also used by Phases 5–6):

```rust
/// In-flight/delivered tool_call_id dedup across concurrent paths
/// (monitor hook vs register_or_complete vs Phase 6 recovery scan).
#[derive(Default)]
pub struct AnswerGuards { answered: tokio::sync::Mutex<std::collections::HashSet<String>> }

impl AnswerGuards {
    /// Check has_tool_result + in-memory set, mark, answer, unmark-on-error.
    /// Returns Ok(true) when the answer was sent, Ok(false) when skipped as duplicate.
    pub async fn answer_once(
        &self, client: &ChatClient, chat_id: i64, tool_call_id: &str, content: String,
    ) -> Result<bool, ClientError>;
}
```

`answer_once` order: lock → if in set → false → insert → drop lock → `has_tool_result` (if true → remove from set, return false) → `answer_tool_call` (on Err: remove from set, propagate) → true. The set + persisted check together make double answers impossible across paths.

### 3. `plugins/rhd_plugin_sub_chat/src/watcher.rs` (new)

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct Waiter { pub parent_chat_id: i64, pub tool_call_id: String }

pub struct Watcher {
    client: Arc<ChatClient>,
    guards: Arc<AnswerGuards>,
    waiters: RwLock<HashMap<i64, Vec<Waiter>>>,
}

impl Watcher {
    pub fn new(client: Arc<ChatClient>, guards: Arc<AnswerGuards>) -> Self;

    /// Register, then immediately evaluate: the subchat may already be completed
    /// (fast mock answers, human-seeded history) and no further state-change event
    /// would ever wake the waiter otherwise. Answers via answer_once (content =
    /// final_answer, "" fallback).
    pub async fn register_or_complete(&self, sub_chat_id: i64, waiter: Waiter) {
        // 1. insert waiter into map (write lock).
        // 2. fetch state: get_chat → chat_state_from_get(&result) (pub(crate) helper here).
        // 3. if completion::is_completed(&state) → self.drain_and_answer(sub_chat_id, &state).await
    }

    /// Atomic take + answer. Called from the monitor hook and register_or_complete.
    pub(crate) async fn drain_and_answer(&self, sub_chat_id: i64, state: &ChatState) {
        // waiters = write-lock.take(sub_chat_id) (remove key entirely — single taker)
        // content = completion::final_answer(state).unwrap_or_default().to_string()
        // for w in waiters: guards.answer_once(client, w.parent_chat_id, &w.tool_call_id, content)
        //   .await — Err → tracing::error! (deleted parent/chat: drop, AD-7); never re-queue.
    }

    pub async fn watched_ids(&self) -> Vec<i64>; // observability/tests
}

/// Install the single ChatMonitor hook fanning into the watcher.
pub async fn install(chat_monitor: &ChatMonitor, watcher: Arc<Watcher>) {
    chat_monitor.on_chat_state_change(move |chat_id, state| {
        let watcher = Arc::clone(&watcher);
        // NEVER await inline in dispatch path (memory/development.md):
        tokio::spawn(async move {
            // cheap pre-filter under read lock: is the chat watched at all?
            // if yes AND completion::is_completed(&state) → drain_and_answer.
        });
    }).await;
}
```

`chat_state_from_get(result: &GetChatResult) -> ChatState`: `ChatState { chat_id, messages: result.messages, queued_messages_count: result.queued_messages_count, tags: result.chat.tags, version: result.chat.version }`.

### 4. `plugins/rhd_plugin_sub_chat/src/handler.rs` (modify)

- `HandlerCtx` gains `pub watcher: Arc<Watcher>`.
- Replace the Phase 3 sync seam (`// NOTE(Phase 4)` marker) with:

```rust
} else {
    ctx.watcher.register_or_complete(
        sub_chat_id,
        Waiter { parent_chat_id: caller_chat_id, tool_call_id: plan.tool_call_id.clone() },
    ).await;
}
```

- All answers (validation errors, async notice) now go through `ctx.guards`/`AnswerGuards::answer_once` for consistency — add `guards: Arc<AnswerGuards>` to `HandlerCtx`.

### 5. `plugins/rhd_plugin_sub_chat/src/plugin.rs` (modify)

Construct once and thread through:

```rust
let guards = Arc::new(AnswerGuards::default());
let watcher = Arc::new(Watcher::new(Arc::clone(&client), Arc::clone(&guards)));
handler::install-into HandlerCtx { client, watcher: Arc::clone(&watcher), guards };
watcher::install(&chat_monitor, Arc::clone(&watcher)).await;   // before keep-alive
```

### 6. `plugins/rhd_plugin_sub_chat/src/lib.rs` (modify)

Add `pub mod completion; pub mod watcher;`.

## Tests

**Unit — `completion.rs` (exhaustive predicate table):** empty messages → false; last=user → false; last=assistant streaming → false; last=assistant not-finished → false; last=assistant finished **with tool_calls** → false; queue 1 → false; tag `ai_completions:running` → false; tag `ai_completions:error` → false; clean finished assistant last → true; `final_answer` returns verbatim content incl. empty string.

**Unit — `watcher.rs` inline (pure parts only):** `chat_state_from_get` field mapping; take-before-answer atomicity simulated by two concurrent `drain_and_answer` on a stub — keep light; the real convergence behavior lands in the integration test below.

**Integration — `plugins/rhd_plugin_sub_chat/tests/sync_spawn_test.rs`** (chat server + this plugin; no ai_completions/mock):
1. Drive `handle_spawn` (async=false, seeded exactly like Phase 3's test) → assert subchat B created/activated and A's call **still unanswered**.
2. Simulate the subchat's lifecycle: `add_message` (B, user copy? no — B's queue already drained in a real flow; here just) append a finished `assistant` message "SUBCHAT FINAL ANSWER" to B → monitor state-change fires → poll A's history until a `tool` message with our `tool_call_id` appears → assert content is byte-identical `"SUBCHAT FINAL ANSWER"`.
3. Negative intermediate states: before step 2, push a `user` message as last → event fires → still unanswered (predicate false).
4. Duplicate event safety: re-add/remove a tag on B twice quickly around completion → exactly one tool answer for the call (`has_tool_result` in A counts 1).
5. Multiple waiters: register a second waiter for the same B via `Watcher::register_or_complete` (different tool_call_id) before step 2 → after completion both answered with the same content.

## Implementation Notes

1. **The register-then-check race (why `register_or_complete` exists):** the completion state-change can land *between* `converge_spawn` returning and the watcher registering (mock providers are instant). Insert-then-evaluate closes it; the alternative (evaluate-then-insert) leaves a permanently parked call. Both paths converge on the atomic `take`.
2. **Predicate is re-checked from event-carried `ChatState`** — the monitor already guarantees version-consistent refetch semantics (`chat_monitor.rs` gap handling); do not refetch inside the hook.
3. **A parent that was deleted while awaiting**: `answer_once` → `add_message` error → logged, waiter dropped. No retry queue — matches "deletion is the user's problem".
4. **No `running`-tag assumption on B:** the predicate tolerates B being answered synthetically (tests) or by ai_completions (production); it never inspects who wrote the assistant message.
5. **Ordering nuance in `ai_request.rs`:** `ai_completions:running` is removed *before* the final `update_message` — there is a window where the message is still `is_finished=false`. The predicate requires both conditions, so no premature completion is possible from this ordering. Don't "optimize" by dropping the is_finished check.

## Dependencies

- **Requires:** Phase 3 (sync seam, `reply.rs`).
- **Blocks:** Phase 5 (status/await reuse `completion.rs` + `register_or_complete`) and Phase 6 (recovery registers waiters through the same API).
