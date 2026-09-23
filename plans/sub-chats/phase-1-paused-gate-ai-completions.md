# Phase 1: `paused` Chat Tag — Gate in `ai_completions`

> Parent plan: [`rhd-plugin-sub-chat-grand-plan.md`](../rhd-plugin-sub-chat-grand-plan.md) (AD-3)

## Overview

Make the bare chat tag `paused` a platform-level stop switch for AI completions: a chat carrying `paused` never triggers an AI request (neither queued-messages nor tool-loop triggers). Removing the tag bumps the chat version, the `ChatMonitor` state-change flow fires, and normal processing resumes. This is independently useful (operator/UI control) and is the foundation the sub-chat plugin's two-phase activation relies on in Phase 3.

**Scope in:** trigger-gate change in `rhd_plugin_ai_completions`, unit tests, one integration test.
**Scope out:** everything else — no server/DB/frontend changes; the tag is not enforced anywhere except `trigger_detection::should_trigger`.

**Dependencies:** none. Parallel with Phase 2. Required before Phase 3 e2e validation (Phase 3's spawn protocol assumes a paused chat cannot start processing).

## Wire Contract (fixed — other phases rely on this)

| Item | Value |
|---|---|
| Tag | exact string `paused` (no namespace prefix) |
| Effect | `ai_completions` returns `TriggerReason::None` while the tag is present |
| Resume | remove tag via `updateChat { removeTags: ["paused"] }` → state change triggers normally |
| Startup reconciliation | unaffected — `tag_crashed_chats` still parks crashed chats with `ai_completions:error` even when paused (intentional: parking is a tag change, not a request) |

## Files to Modify

### 1. `plugins/rhd_plugin_ai_completions/src/trigger_detection.rs`

**Modify `should_trigger`** — add the paused check right after the error-tag check (same early-return shape):

```rust
pub fn should_trigger(chat_state: &ChatState) -> TriggerReason {
    // Skip if chat has error tag
    if has_error_tag(chat_state) {
        return TriggerReason::None;
    }

    // Skip if chat is paused (platform-level stop switch, see grand plan AD-3)
    if has_paused_tag(chat_state) {
        return TriggerReason::None;
    }
    // ... existing queued-messages and tool-loop checks unchanged
}
```

**Add helper** next to the existing `has_error_tag`:

```rust
/// Check if chat has the `paused` tag (platform-level: no AI requests while paused).
pub fn has_paused_tag(chat_state: &ChatState) -> bool {
    chat_state.tags.iter().any(|tag| tag == "paused")
}
```

**Imports:** none new.

### 2. `plugins/rhd_plugin_ai_completions/tests/integration_test.rs`

**Add test `test_paused_chat_never_triggers_until_unpaused`** following the `TestEnv` pattern already in this file (chat server + mock AI + in-process plugin):

1. `create_chat` with `tags: vec!["paused".to_string()]`.
2. Register the test connection as a plugin (needed for `updateChat` tag ops? — no; plain `update_chat` works from any connection, verify while writing).
3. `add_queue_message` (user, "Hello") → wait ~2 s → assert `listener.get_requests().is_empty()`.
4. `update_chat(UpdateChatParams { chat_id, add_tags: vec![], remove_tags: vec!["paused".into()], .. })`.
5. Poll `get_chat` until an assistant message with `is_finished` appears (use the existing wait helper in this test file); assert the mock received exactly one request and the answer text matches the pushed response.

No production code changes in the test file's harness.

## Tests

### Unit tests in `trigger_detection.rs` (inline `mod tests`)

Reuse the file's existing `create_chat_state` helper:

- `paused_blocks_queued_messages_trigger` — queue>0, tags `["paused"]` → `None`.
- `paused_blocks_tool_loop_continuation` — fully-resolved tool loop, `paused` → `None`.
- `has_paused_tag_positive_negative` — `["paused"]` true; `["sub_chat:paused"]`, `[]` false (exact match only).
- Unpause path is the existing trigger tests (no `paused` tag) — already covered.

## Implementation Notes

1. **Exact-match tag, no namespace:** `paused` is deliberately bare (AD-3) — it is a user-facing platform control the UI can toggle, not plugin-private state. Do **not** accept variants like `sub_chat:paused`.
2. **Gate placement matters:** check after the error tag so `has_error_tag` semantics and its tests stay untouched. Precedence between the two tags is irrelevant (both → `None`).
3. **Nothing else reacts to `paused`:** system-prompt injection, MCP registration, todo contracts all continue working on paused chats (they don't emit AI requests). The sub-chat plugin relies on this: tags/prompts land while paused, first request happens only after unpause.
4. **No running-tag interaction:** a chat cannot be simultaneously `paused` and have an in-flight request started after this change — the only writer of `ai_completions:running` is the trigger path, which is now gated. If an operator pauses mid-flight, the in-flight request completes normally; only the *next* trigger is blocked. Document this in the plugin README? Not needed here — README covered in Phase 8.

## Dependencies

- **No prerequisites.** Start immediately.
- **Blocks:** Phase 3 e2e validation and Phase 7 (the two-phase activation protocol and the "paused never triggers" assertion). Phase 2 is independent and runs in parallel.
