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
