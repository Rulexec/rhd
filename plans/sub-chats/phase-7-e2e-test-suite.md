# Phase 7: End-to-End Test Suite (Full Pipeline, Mock AI)

> Parent plan: [`rhd-plugin-sub-chat-grand-plan.md`](../rhd-plugin-sub-chat-grand-plan.md)

## Overview

Validate the assembled feature the way production runs it: chat server + `rhd_plugin_ai_completions` + `rhd_plugin_sub_chat` + one mock AI provider, with a **content-routing listener** so parent-chat and sub-chat model requests get deterministic scripted responses regardless of interleaving. This is the only phase that exercises real trigger timing, the tool-loop parking mechanism, cross-plugin coordination (ack flows), and a true restart-recovery story — none of which the Phase 3–6 integration tests (which drive plugin functions synthetically) can reach.

**Scope in:** shared e2e harness for this plugin, scenario tests listed below.
**Scope out:** new production behavior. If a scenario exposes a bug, fix in the owning phase's module (that fix is part of this phase's deliverable); never weaken an assertion to pass.

**Dependencies:** Phases 1–6 complete.

## Wire Contract (test-visible, fixed)

All strings come from earlier phases and are asserted byte-exact here: `async_notice` text, `pending`/`completed`, `sub · ` title prefix, tag sets, predicate behavior.

## Files to Create

### 1. `plugins/rhd_plugin_sub_chat/tests/common/mod.rs` (new)

Extend the `TestEnv` pattern from `plugins/rhd_plugin_ai_completions/tests/tool_call_e2e_test.rs` (copy its in-memory-server + temp-credentials/config bootstrap) with:

```rust
/// Content-routed mock listener. `MockAiListener` is a public trait —
/// implement it in-test (SimpleListener/RecordingListener/DynamicListener are all FIFO
/// and would interleave nondeterministically with two concurrent chats).
pub struct RoutingListener {
    // route key = first matching marker among configured routes; per-route VecDeque<MockAiResponse>
    routes: Mutex<Vec<(String /*marker substring*/, VecDeque<MockAiResponse>)>>,
    requests: Mutex<Vec<ChatCompletionRequest>>,   // for "never received" assertions
}
impl RoutingListener {
    pub fn register(&self, marker: &str, responses: Vec<MockAiResponse>);
    pub fn take_requests(&self) -> Vec<ChatCompletionRequest>;
}
// on_chat_completion: find the route whose marker occurs in any request message content
// (check `content` of system/user/assistant/tool parts); pop_front its next response;
// default route "fallback" responds MockAiResponse::text("unexpected request").

pub struct FullEnv { /* chat server, mock provider (RoutingListener), both plugins spawned */ }
impl FullEnv {
    pub async fn start() -> Self;          // spawns ai_completions::run_plugin and
                                           // rhd_plugin_sub_chat::plugin::run_plugin tasks
    pub async fn plugin_client(&self) -> ChatClient; // registered helper connection: acks
                                           // custom events, usable for add_message/update_chat
    pub async fn wait_for<T>(&self, what: &str, f: impl FnMut() -> BoxFuture<bool>) -> ();
    pub async fn messages(&self, chat_id: i64) -> Vec<Message>;
    pub async fn chat(&self, chat_id: i64) -> rhd_chat_api::GetChatResult;
}
```

Marker convention: sub-chat seed messages contain unique task ids (`"E2E-TASK-A"`); parent requests contain other text. Register the parent route to return `MockAiResponse::stream_tool_call("rhd_sub_chat", r#"{"messages":[{"role":"user","content":"E2E-TASK-A ..."}],"async":true/false}"#)`, then a final text response — and note the parent's *second* request (after the tool answer lands) re-contains the marker, so routes match on request **shape** (last message role) before substring: parent-route matches "no assistant answer for the newest tool call yet", subchat-route matches "newest message is the seeded user". Document the chosen matcher in code comments; keep it dumb and deterministic. Dev-deps to add in `Cargo.toml`: `rhd_mock_ai_provider`, `rhd_plugin_ai_completions`, `tempfile`, `futures-util`.

### 2. `plugins/rhd_plugin_sub_chat/tests/e2e_spawn_sync_test.rs`

**Scenario 1 — full sync round trip (the flagship):**
1. Create parent chat A (test client), queue user msg "do it" → parent route #1: text answer **with** `rhd_sub_chat(async:false)` tool call... simpler: route returns `MockAiResponse::tool_call(...)`/`stream_tool_call` directly.
2. ai_completions stores the assistant call → sub_chat plugin spawns B (assert: exists; tags exactly `[parent:A, root:A, sub_chat:call:<tcid>]` — capture `<tcid>` from B's stored assistant message in A; `paused` gone; queue drained by ai_completions).
3. Subchat route answers `"E2E-SUBTASK-A"…` → mock returns final text `"SUB ANSWER A"`.
4. Assert A receives exactly one `tool` message for `<tcid>` with content `"SUB ANSWER A"` **before** ai_completions' next parent request carries the tool result (order via chat version growth; use `wait_for`).
5. Parent route #2 returns plain text → assert final parent assistant exists (loop demonstrably resumed).
6. Assert `get_requests()` shows the subchat's request built while `paused` never happened: no provider request existed between B creation and unpause — verify via request count snapshots around the unpause step? (Simpler equivalent: assert B's request count is exactly 1 for its single task.)

**Scenario 2 — async + status + await ladder:** same spawn with `async:true` → assert exact `async_notice(B)` text; parent route continues immediately (answer = notice); model calls `rhd_sub_chat_status` while subchat still streaming (route the status-call answer as text `"E2E-TASK-B"` pending) → `pending`; then `rhd_sub_chat_await` → holds; subchat answers → await answered verbatim; parent final turn.

**Scenario 3 — parked subchat:** subchat route returns `MockAiResponse::Error{500,...}` → B parked with `ai_completions:error`; parent `status` → `pending`; assert no repeated provider requests (parking works); operator: test client `update_chat` removes the error tag, queues "try again" on B; route now succeeds; parent `status` flips to `completed`, fresh `await` answers. (Document: un-parking alone doesn't retrigger — a new queued message does, matching ai_completions semantics.)

### 3. `plugins/rhd_plugin_sub_chat/tests/e2e_nesting_scopes_test.rs`

**Scenario 4 — nesting:** B's assistant (driven by subchat route) calls `rhd_sub_chat(async:true)` spawning C: assert C tags `[parent:B, root:A, sub_chat:call:<tc>]` (root **inherited**, not B); B completes after C's answer flows in (sync nested spawn in B's call args — two nested waits in one scenario).
**Scenario 5 — scope enforcement from the model's seat:** A calls `rhd_sub_chat_status` with C's id → error text `not a direct subchat`; await on C from A likewise. A's loop demonstrably continues after the error answers.
**Scenario 6 — validation from the model's seat:** parent route emits `rhd_sub_chat` with `messages:[{"role":"assistant",...}]` → error answer, **no new chat created** (list_chats delta zero).

### 4. `plugins/rhd_plugin_sub_chat/tests/e2e_recovery_test.rs`

**Scenario 7 — true crash story with the real pipeline:**
1. Parent route emits `rhd_sub_chat(async:false)`; subchat route is registered to answer.
2. Kill the sub_chat plugin task (drop its cancellation / restart semantics: `run_plugin` has no graceful exit — wrap spawn in a `JoinHandle` + internal-shutdown test hook is NOT allowed to add production surface; instead simulate crash the honest way: stop the whole *server*, or start scenario with plugin **not** spawned and rely on recovery: emit the parent tool call, and pre-create the crash state the same way Phase 6's test does but using the **real** ai_completions to write the assistant tool_calls message; then spawn the sub_chat plugin and assert recovery converges from `paused`+partial-queue (manually created) to answered.)
   Choose the variant implementable without touching production code — the manual-state + fresh-plugin-start approach — and document which crash windows are covered by Phase 6's unit/integration tests vs. this real-history variant.
3. Assert exactly-once answering end-to-end (tool message count for `<tcid>` == 1 in A's final history).

**Scenario 8 — ack-flow guard:** with the sub_chat plugin registered and idle, a plain user chat still completes without `preRequest` timeout (cheap global regression; mirrors `rhd_plugin_choice/tests/custom_event_ack_test.rs` intent but at full-pipeline level).

## Implementation Notes

1. **File-size budget:** three test files + common — each under the 400-line project threshold; split further by scenario if needed (code-splitting skill convention).
2. **Timing assertions** must be `wait_for`-style polls with generous timeouts (10–30 s), never fixed sleeps, to avoid flakes in CI — the existing ai_completions e2e tests already follow this; copy their helper idiom.
3. **Two plugins, one mock, in-memory db:** both plugins registered with distinct ids (`test_ai`, `test_sub_chat`); the test client registers as a third and acks all custom events (choice/Phase-2 test precedent) — otherwise ai_completions stalls.
4. **`stream_tool_call` vs `tool_call`:** prefer the non-streaming variant for determinism unless the phase under test is streaming merge behavior (it isn't).
5. If RoutingListener matching proves fiddly, the escape hatch: **two mock providers** with ai_completions pointed at one… no — one plugin instance serves all chats; the real fallback is marker design discipline (unique per chat, present in seed only). Keep RoutingListener.

## Dependencies

- **Requires:** all previous phases; blocks nothing except Phase 8's finalization.
