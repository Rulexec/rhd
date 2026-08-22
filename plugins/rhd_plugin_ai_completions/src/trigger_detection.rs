//! Trigger condition detection for AI completions.
//!
//! This module analyzes chat state to determine whether an AI completion
//! should be triggered, and for what reason.

use rhd_chat_client::ChatState;

use crate::tool_resolution;

/// Reason for triggering AI completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerReason {
    /// Queued messages present, no unresolved tool calls.
    QueuedMessages,
    /// Tool loop continuation: last assistant has tool calls, all resolved.
    ToolLoopContinuation,
    /// No trigger needed.
    None,
}

/// Check if chat should trigger AI completion.
///
/// The detection logic follows this priority:
/// 1. If chat has error tag → no trigger
/// 2. If queued messages present and no unresolved tool calls → QueuedMessages
/// 3. If all tool calls resolved → ToolLoopContinuation
/// 4. Otherwise → None
pub fn should_trigger(chat_state: &ChatState) -> TriggerReason {
    // Skip if chat has error tag
    if has_error_tag(chat_state) {
        return TriggerReason::None;
    }

    // Check for queued messages mode
    if chat_state.queued_messages_count > 0
        && !tool_resolution::has_unresolved_tool_calls(&chat_state.messages)
    {
        return TriggerReason::QueuedMessages;
    }

    // Check for tool loop continuation mode
    if tool_resolution::all_tool_calls_resolved(&chat_state.messages) {
        return TriggerReason::ToolLoopContinuation;
    }

    TriggerReason::None
}

/// Check if chat has the `ai_completions:error` tag.
pub fn has_error_tag(chat_state: &ChatState) -> bool {
    chat_state
        .tags
        .iter()
        .any(|tag| tag == "ai_completions:error")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rhd_chat_api::Message;

    fn create_message(id: i64, role: &str, content: &str) -> Message {
        Message {
            id,
            chat_id: 1,
            role: role.to_string(),
            content: content.to_string(),
            created_at: Utc::now(),
            reasoning_content: None,
            tags: vec![],
        }
    }

    fn create_chat_state(
        messages: Vec<Message>,
        queued_count: i64,
        tags: Vec<String>,
    ) -> ChatState {
        ChatState {
            chat_id: 1,
            messages,
            queued_messages_count: queued_count,
            tags,
        }
    }

    #[test]
    fn test_no_trigger_empty_chat() {
        let state = create_chat_state(vec![], 0, vec![]);
        assert_eq!(should_trigger(&state), TriggerReason::None);
    }

    #[test]
    fn test_trigger_queued_messages() {
        let messages = vec![
            create_message(1, "user", "Hello"),
            create_message(2, "assistant", "Hi"),
        ];
        let state = create_chat_state(messages, 1, vec![]);
        assert_eq!(should_trigger(&state), TriggerReason::QueuedMessages);
    }

    #[test]
    fn test_no_trigger_error_tag() {
        let messages = vec![create_message(1, "user", "Hello")];
        let state = create_chat_state(messages, 1, vec!["ai_completions:error".to_string()]);
        assert_eq!(should_trigger(&state), TriggerReason::None);
    }

    #[test]
    fn test_trigger_tool_loop_continuation() {
        let messages = vec![
            create_message(1, "user", "Use a tool"),
            create_message(2, "assistant", "Using tool"),
            create_message(3, "tool", "Tool result"),
        ];
        let state = create_chat_state(messages, 0, vec![]);
        assert_eq!(should_trigger(&state), TriggerReason::ToolLoopContinuation);
    }

    #[test]
    fn test_queued_messages_takes_priority_over_tool_loop() {
        let messages = vec![
            create_message(1, "user", "Use a tool"),
            create_message(2, "assistant", "Using tool"),
            create_message(3, "tool", "Tool result"),
        ];
        let state = create_chat_state(messages, 2, vec![]);
        assert_eq!(should_trigger(&state), TriggerReason::QueuedMessages);
    }

    #[test]
    fn test_has_error_tag_positive() {
        let state = create_chat_state(vec![], 0, vec!["ai_completions:error".to_string()]);
        assert!(has_error_tag(&state));
    }

    #[test]
    fn test_has_error_tag_negative() {
        let state = create_chat_state(vec![], 0, vec!["other-tag".to_string()]);
        assert!(!has_error_tag(&state));
    }

    #[test]
    fn test_has_error_tag_empty() {
        let state = create_chat_state(vec![], 0, vec![]);
        assert!(!has_error_tag(&state));
    }
}
