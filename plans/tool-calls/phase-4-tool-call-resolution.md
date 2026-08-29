# Phase 4: Real Tool-Call Resolution — the D4 Trigger Gate (milestone P5)

## Overview

Replace the heuristic stubs in
[`tool_resolution.rs`](../../plugins/rhd_plugin_ai_completions/src/tool_resolution.rs:17)
with real id matching. Today `has_unresolved_tool_calls` always returns `false` and
`all_tool_calls_resolved` returns `count > 0`, so the D4 gate — **no request while tool
calls are unresolved** — does not exist. This phase creates it.

This is a **hard prerequisite for phase-5**: once `build_chat_messages` forwards assistant
`tool_calls`, a chat caught mid-loop must simply *not trigger* (it keeps polling until a
tool-executing plugin posts results), instead of being parked with an error.

Semantics (from milestone P5):

- `has_unresolved_tool_calls`: ids declared by the **last** assistant message minus ids
  answered by subsequent `tool` messages → non-empty means unresolved.
- `all_tool_calls_resolved`: the declared set is **non-empty** **and** the unresolved set
  is empty.

## Dependencies

- **Requires phase-2** (`rhd_chat_api::Message.tool_call_id` exists on `tool` messages).
- **Blocks phase-5** (converter assumes the gate exists; ordering note in milestone
  "Risks / Notes").

## Files to Modify

### 1. `plugins/rhd_plugin_ai_completions/src/tool_resolution.rs` — full rewrite

Replace the entire file with:

```rust
//! Tool call resolution detection.
//!
//! This module detects when the tool calls of the last assistant message have been
//! resolved by corresponding `tool`-role result messages. A tool call is resolved when
//! a `tool` message appearing *after* the declaring assistant message carries a
//! matching `tool_call_id`.
//!
//! This is the D4 trigger gate: while `has_unresolved_tool_calls` returns true, the
//! plugin must not build or send a request — the conversation is in progress, and the
//! plugin keeps polling until every declared id is answered.

use std::collections::HashSet;

use rhd_chat_api::Message;

/// Inspect the tool loop of the last assistant message.
///
/// Returns `(declared, unresolved)`: `declared` are the ids of the tool calls made by
/// the last assistant message (empty if it made none), and `unresolved` is the subset
/// not yet answered by a `tool` message after it.
fn last_assistant_tool_loop(messages: &[Message]) -> (Vec<String>, Vec<String>) {
    let Some(last_assistant_idx) = messages.iter().rposition(|m| m.role == "assistant") else {
        return (Vec::new(), Vec::new());
    };

    let declared: Vec<String> = messages[last_assistant_idx]
        .tool_calls
        .iter()
        .map(|tool_call| tool_call.id.clone())
        .collect();
    if declared.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let answered: HashSet<String> = messages[last_assistant_idx + 1..]
        .iter()
        .filter(|m| m.role == "tool")
        .filter_map(|m| m.tool_call_id.clone())
        .collect();

    let unresolved = declared
        .iter()
        .filter(|id| !answered.contains(id.as_str()))
        .cloned()
        .collect();

    (declared, unresolved)
}

/// Check if there are unresolved tool calls in the message history.
///
/// Returns true if the last assistant message has tool calls that don't have
/// corresponding tool result messages.
pub fn has_unresolved_tool_calls(messages: &[Message]) -> bool {
    let (_declared, unresolved) = last_assistant_tool_loop(messages);
    !unresolved.is_empty()
}

/// Check if all tool calls from the last assistant message are resolved.
///
/// Returns true only when the last assistant message declared at least one tool
/// call and every declared id has a matching tool result message after it.
pub fn all_tool_calls_resolved(messages: &[Message]) -> bool {
    let (declared, unresolved) = last_assistant_tool_loop(messages);
    !declared.is_empty() && unresolved.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rhd_chat_api::{FunctionCall, ToolCall};

    fn message(id: i64, role: &str, content: &str) -> Message {
        Message {
            id,
            chat_id: 1,
            role: role.to_string(),
            content: content.to_string(),
            tool_call_id: None,
            created_at: Utc::now(),
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
            tool_calls: vec![],
        }
    }

    fn assistant_with_calls(id: i64, call_ids: &[&str]) -> Message {
        let mut msg = message(id, "assistant", "");
        msg.tool_calls = call_ids
            .iter()
            .map(|call_id| ToolCall {
                id: call_id.to_string(),
                call_type: "function".to_string(),
                function: FunctionCall {
                    name: "get_weather".to_string(),
                    arguments: "{}".to_string(),
                },
                tags: vec![],
            })
            .collect();
        msg
    }

    fn tool_result(id: i64, tool_call_id: &str) -> Message {
        let mut msg = message(id, "tool", "result");
        msg.tool_call_id = Some(tool_call_id.to_string());
        msg
    }

    #[test]
    fn no_assistant_messages_has_no_unresolved_and_is_not_resolved() {
        let messages = vec![message(1, "user", "Hello")];
        assert!(!has_unresolved_tool_calls(&messages));
        assert!(!all_tool_calls_resolved(&messages));
    }

    #[test]
    fn assistant_without_tool_calls_needs_nothing() {
        let messages = vec![
            message(1, "user", "Hello"),
            message(2, "assistant", "Hi there"),
        ];
        assert!(!has_unresolved_tool_calls(&messages));
        assert!(!all_tool_calls_resolved(&messages));
    }

    #[test]
    fn single_call_unresolved_until_result_arrives() {
        let pending = vec![
            message(1, "user", "Weather?"),
            assistant_with_calls(2, &["call_1"]),
        ];
        assert!(has_unresolved_tool_calls(&pending));
        assert!(!all_tool_calls_resolved(&pending));

        let answered = vec![
            message(1, "user", "Weather?"),
            assistant_with_calls(2, &["call_1"]),
            tool_result(3, "call_1"),
        ];
        assert!(!has_unresolved_tool_calls(&answered));
        assert!(all_tool_calls_resolved(&answered));
    }

    #[test]
    fn two_calls_one_result_is_still_unresolved() {
        let messages = vec![
            message(1, "user", "Use tools"),
            assistant_with_calls(2, &["call_a", "call_b"]),
            tool_result(3, "call_a"),
        ];
        assert!(has_unresolved_tool_calls(&messages));
        assert!(!all_tool_calls_resolved(&messages));
    }

    #[test]
    fn two_calls_two_results_is_resolved() {
        let messages = vec![
            message(1, "user", "Use tools"),
            assistant_with_calls(2, &["call_a", "call_b"]),
            tool_result(3, "call_b"),
            tool_result(4, "call_a"),
        ];
        assert!(!has_unresolved_tool_calls(&messages));
        assert!(all_tool_calls_resolved(&messages));
    }

    #[test]
    fn results_for_earlier_loops_do_not_count() {
        // A completed loop followed by a plain assistant answer: nothing pending.
        let messages = vec![
            assistant_with_calls(1, &["call_a"]),
            tool_result(2, "call_a"),
            message(3, "assistant", "Here is the summary"),
        ];
        assert!(!has_unresolved_tool_calls(&messages));
        assert!(!all_tool_calls_resolved(&messages));
    }

    #[test]
    fn orphan_result_does_not_resolve_a_declared_call() {
        let messages = vec![
            assistant_with_calls(1, &["call_a"]),
            tool_result(2, "call_unrelated"),
        ];
        assert!(has_unresolved_tool_calls(&messages));
        assert!(!all_tool_calls_resolved(&messages));
    }

    #[test]
    fn duplicate_result_for_one_call_leaves_the_other_unresolved() {
        let messages = vec![
            assistant_with_calls(1, &["call_a", "call_b"]),
            tool_result(2, "call_a"),
            tool_result(3, "call_a"),
        ];
        assert!(has_unresolved_tool_calls(&messages));
        assert!(!all_tool_calls_resolved(&messages));
    }

    #[test]
    fn tool_message_without_id_does_not_resolve_anything() {
        let mut result = message(3, "tool", "result");
        result.tool_call_id = None;
        let messages = vec![assistant_with_calls(1, &["call_a"]), result];
        assert!(has_unresolved_tool_calls(&messages));
    }
}
```

> `rhd_chat_api::{FunctionCall, ToolCall}` are re-exported at the crate root (already used
> as `rhd_chat_api::ToolCall` in `ai_request.rs:344`).

### 2. `plugins/rhd_plugin_ai_completions/src/trigger_detection.rs` — tests only

`should_trigger` itself needs **no change** — it already refuses both branches via
`has_unresolved_tool_calls` / `all_tool_calls_resolved` ([lines 35–44](../../plugins/rhd_plugin_ai_completions/src/trigger_detection.rs:35)).
With phase-4's real resolution it finally means what the docs claim.

Replace the heuristic fixtures/tests (milestone P5: existing heuristic tests are
**replaced, not kept**):

- The `create_message` fixture already carries `tool_call_id: None` (added in phase-2).
  Add the same `assistant_with_calls` / `tool_result` helpers as in `tool_resolution.rs`
  tests (duplicate the small helpers; they are test-only).
- **Replace** `test_trigger_tool_loop_continuation` with a real fixture:

```rust
#[test]
fn test_trigger_tool_loop_continuation() {
    let messages = vec![
        create_message(1, "user", "Use tools"),
        assistant_with_calls(2, &["call_a", "call_b"]),
        tool_result(3, "call_a"),
        tool_result(4, "call_b"),
    ];
    let state = create_chat_state(messages, 0, vec![]);
    assert_eq!(should_trigger(&state), TriggerReason::ToolLoopContinuation);
}
```

- **Add the D4 gate test** (milestone P5's key acceptance case):

```rust
#[test]
fn test_no_trigger_while_tool_calls_unresolved() {
    // Last assistant declares 2 calls; only 1 tool result posted.
    let messages = vec![
        create_message(1, "user", "Use tools"),
        assistant_with_calls(2, &["call_a", "call_b"]),
        tool_result(3, "call_a"),
    ];
    let state = create_chat_state(messages, 0, vec![]);
    assert_eq!(should_trigger(&state), TriggerReason::None);
}
```

- **Update** `test_queued_messages_takes_priority_over_tool_loop` to use the fully-resolved
  two-call fixture plus `queued_messages_count: 2` → still `QueuedMessages`.
- **Add** the combined case: queued messages present **and** unresolved calls →
  `TriggerReason::None` (the queued branch is gated on `!has_unresolved_tool_calls`):

```rust
#[test]
fn test_queued_messages_still_gated_on_resolution() {
    let messages = vec![
        create_message(1, "user", "Use tools"),
        assistant_with_calls(2, &["call_a", "call_b"]),
        tool_result(3, "call_a"),
    ];
    let state = create_chat_state(messages, 1, vec![]);
    assert_eq!(should_trigger(&state), TriggerReason::None);
}
```

- `test_no_trigger_empty_chat`, `test_trigger_queued_messages`, `test_no_trigger_error_tag`,
  and the `has_error_tag` tests stay as-is (they don't depend on the heuristic).

## Implementation Notes

1. **Why "last assistant message" only**: the trigger asks "may I ask the model for its
   next turn now?" — that depends only on the loop currently open. Older loops were
   already complete when the next assistant message was produced. Full-history integrity
   is validated separately in phase-5's converter.
2. **`rposition` over `rev()` + `position`**: the old code searched for the last assistant
   and then re-found its index; `rposition` does both in one pass.
3. **A `tool` message without `tool_call_id` answers nothing** (the `filter_map` skips it).
   Phase-2's server validation makes such messages uncreatable going forward; the converter
   (phase-5) refuses to build a request over history that predates the rule.
4. **Invariant to preserve** (milestone D4): every `TriggerReason` branch in `should_trigger`
   must stay gated on resolution. Any new reason added later must respect it.
5. **Behavior change to expect**: chats whose assistant has tool calls now stop triggering
   until results are posted. No tool-executing plugin exists yet, so such chats idle —
   that is the correct, safe behavior this plan intends.

## Validation

```bash
mise run check-cargo
mise run test-cargo
```
