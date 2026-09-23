//! The single source of truth for "the subchat is done" (grand plan AD-2).
//!
//! Pure predicate over a [`ChatState`] — no client calls, no side effects —
//! so the watcher hook, `register_or_complete`, and Phase 5's status tool all
//! evaluate completion through exactly one encoding.

use rhd_chat_api::Message;
use rhd_chat_client::ChatState;

/// Status answer for a subchat that is not completed (Phase 5, AD-6).
pub const PENDING: &str = "pending";
/// Status answer for a completed subchat (Phase 5, AD-6).
pub const COMPLETED: &str = "completed";

/// The completion predicate (AD-2): last message is a finished, non-streaming
/// assistant message with no tool calls, the queue is drained, and the chat
/// carries neither `ai_completions:running` nor `ai_completions:error`.
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

/// `running` means a request is in flight; `error` means the loop died with
/// unresolved work — neither state may be reported as "done".
fn has_blocking_tag(tags: &[String]) -> bool {
    tags.iter()
        .any(|t| t == "ai_completions:running" || t == "ai_completions:error")
}

/// Content of the final assistant message when `is_completed` holds.
///
/// **Why "no unresolved tool calls" needs no scan here:** if the *last*
/// message is an assistant message declaring tool calls, no `tool` result can
/// exist *after* it — so `tool_calls.is_empty()` on the last message is
/// exactly equivalent to "last loop resolved" for the completion check. The
/// general unresolved-tool-call scan `ai_completions` uses
/// (`tool_resolution.rs`) is not needed; noted for reviewers comparing the
/// two. Keep this note if the predicate ever stops looking at the last
/// message only.
pub fn final_answer(state: &ChatState) -> Option<&str> {
    state
        .messages
        .last()
        .filter(|m| is_final_assistant(m))
        .map(|m| m.content.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(role: &str, content: &str) -> Message {
        Message {
            id: 1,
            chat_id: 1,
            role: role.to_string(),
            content: content.to_string(),
            tool_call_id: None,
            created_at: "2026-09-23T00:00:00Z".parse().unwrap(),
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
            tool_calls: vec![],
        }
    }

    fn assistant_with_tool_calls() -> Message {
        let mut msg = message("assistant", "working on it");
        msg.tool_calls = vec![rhd_chat_api::ToolCall {
            id: "call_1".to_string(),
            call_type: "function".to_string(),
            function: rhd_chat_api::FunctionCall {
                name: "some_tool".to_string(),
                arguments: "{}".to_string(),
            },
            tags: vec![],
        }];
        msg
    }

    fn state(messages: Vec<Message>, queued: i64, tags: Vec<&str>) -> ChatState {
        ChatState {
            chat_id: 1,
            messages,
            queued_messages_count: queued,
            tags: tags.into_iter().map(String::from).collect(),
            version: 1,
        }
    }

    fn completed_state() -> ChatState {
        state(
            vec![message("user", "q"), message("assistant", "a")],
            0,
            vec![],
        )
    }

    // ---- is_completed: exhaustive predicate table ----

    #[test]
    fn clean_finished_assistant_last_is_completed() {
        assert!(is_completed(&completed_state()));
    }

    #[test]
    fn empty_messages_is_not_completed() {
        assert!(!is_completed(&state(vec![], 0, vec![])));
    }

    #[test]
    fn last_user_message_is_not_completed() {
        assert!(!is_completed(&state(
            vec![message("assistant", "a"), message("user", "q")],
            0,
            vec![]
        )));
    }

    #[test]
    fn last_assistant_streaming_is_not_completed() {
        let mut msg = message("assistant", "half-written");
        msg.is_streaming = true;
        assert!(!is_completed(&state(vec![msg], 0, vec![])));
    }

    #[test]
    fn last_assistant_not_finished_is_not_completed() {
        let mut msg = message("assistant", "half-written");
        msg.is_finished = false;
        assert!(!is_completed(&state(vec![msg], 0, vec![])));
    }

    #[test]
    fn last_assistant_with_tool_calls_is_not_completed() {
        assert!(!is_completed(&state(
            vec![assistant_with_tool_calls()],
            0,
            vec![]
        )));
    }

    #[test]
    fn nonempty_queue_is_not_completed() {
        assert!(!is_completed(&state(
            vec![message("assistant", "a")],
            1,
            vec![]
        )));
    }

    #[test]
    fn running_tag_blocks_completion() {
        assert!(!is_completed(&state(
            vec![message("assistant", "a")],
            0,
            vec!["ai_completions:running"]
        )));
    }

    #[test]
    fn error_tag_blocks_completion() {
        assert!(!is_completed(&state(
            vec![message("assistant", "a")],
            0,
            vec!["ai_completions:error"]
        )));
    }

    #[test]
    fn unrelated_tags_do_not_block_completion() {
        assert!(is_completed(&state(
            vec![message("assistant", "a")],
            0,
            vec!["parent:1", "root:1", "sub_chat:call:x", "paused"]
        )));
    }

    // ---- final_answer ----

    #[test]
    fn final_answer_returns_content_verbatim() {
        let s = state(
            vec![
                message("user", "q"),
                message("assistant", "  spaced\nverbatim answer  "),
            ],
            0,
            vec![],
        );
        assert_eq!(final_answer(&s), Some("  spaced\nverbatim answer  "));
    }

    #[test]
    fn final_answer_returns_empty_string_for_empty_content() {
        let s = state(vec![message("assistant", "")], 0, vec![]);
        assert_eq!(final_answer(&s), Some(""));
    }

    #[test]
    fn final_answer_none_when_not_completed() {
        // Last message is a user one — even with a finished assistant earlier.
        let s = state(
            vec![message("assistant", "a"), message("user", "q")],
            0,
            vec![],
        );
        assert_eq!(final_answer(&s), None);
        assert_eq!(final_answer(&state(vec![], 0, vec![])), None);
    }

    #[test]
    fn final_answer_none_for_assistant_with_tool_calls() {
        assert_eq!(
            final_answer(&state(vec![assistant_with_tool_calls()], 0, vec![])),
            None
        );
    }

    #[test]
    fn status_constants_match_contract() {
        assert_eq!(PENDING, "pending");
        assert_eq!(COMPLETED, "completed");
    }
}
