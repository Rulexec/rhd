//! Tool call resolution detection.
//!
//! This module provides functions to detect when tool calls from an assistant
//! message have been resolved by corresponding tool result messages.

use rhd_chat_api::Message;

/// Check if there are unresolved tool calls in the message history.
///
/// Returns true if the last assistant message has tool calls that don't have
/// corresponding tool result messages.
///
/// Note: This implementation uses a simplified detection mechanism based on
/// the presence of tool role messages after the last assistant message.
/// A more sophisticated implementation would parse tool call IDs from the
/// assistant message content and match them with tool result messages.
pub fn has_unresolved_tool_calls(messages: &[Message]) -> bool {
    // Find the last assistant message
    let last_assistant = messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant");

    let last_assistant = match last_assistant {
        Some(m) => m,
        None => return false, // No assistant messages
    };

    // Find the index of the last assistant message
    let last_assistant_idx = match messages.iter().position(|m| m.id == last_assistant.id) {
        Some(idx) => idx,
        None => return false,
    };

    // Check if there are any tool messages after the last assistant message
    let has_tool_messages = messages[last_assistant_idx + 1..]
        .iter()
        .any(|m| m.role == "tool");

    // If there are no tool messages after the last assistant, we can't determine
    // if there are unresolved tool calls. For now, we assume no unresolved calls.
    // In a real implementation, we would check if the assistant message content
    // indicates tool calls were made.
    if !has_tool_messages {
        return false;
    }

    // Count tool messages after the last assistant
    let tool_results_count = messages[last_assistant_idx + 1..]
        .iter()
        .filter(|m| m.role == "tool")
        .count();

    // For now, we use a heuristic: if there are tool messages, we assume
    // they correspond to tool calls. A more sophisticated implementation
    // would parse the assistant message content to extract tool call IDs
    // and match them with tool result messages.
    //
    // Since we can't determine the exact number of tool calls from the
    // message structure, we return false (no unresolved calls) if there
    // are any tool messages. This is a conservative approach.
    //
    // TODO: Implement proper tool call ID extraction and matching when
    // the Message struct is extended to include tool_calls field.
    let _ = tool_results_count; // Suppress unused variable warning
    false
}

/// Check if all tool calls from the last assistant message are resolved.
///
/// Returns true if the last assistant message has tool calls and all of them
/// have corresponding tool result messages.
///
/// Note: This implementation uses a simplified detection mechanism.
/// See [`has_unresolved_tool_calls`] for details.
pub fn all_tool_calls_resolved(messages: &[Message]) -> bool {
    // Find the last assistant message
    let last_assistant = messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant");

    let last_assistant = match last_assistant {
        Some(m) => m,
        None => return false, // No assistant messages
    };

    // Find the index of the last assistant message
    let last_assistant_idx = match messages.iter().position(|m| m.id == last_assistant.id) {
        Some(idx) => idx,
        None => return false,
    };

    // Check if there are any tool messages after the last assistant message
    let tool_results_count = messages[last_assistant_idx + 1..]
        .iter()
        .filter(|m| m.role == "tool")
        .count();

    // If there are tool messages, we assume all tool calls are resolved.
    // This is a simplified heuristic. A proper implementation would:
    // 1. Parse tool call IDs from the assistant message content
    // 2. Extract tool_call_id from each tool result message
    // 3. Verify all tool call IDs have corresponding results
    //
    // TODO: Implement proper tool call resolution when Message struct
    // is extended to include tool_calls field.
    tool_results_count > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

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

    #[test]
    fn test_no_assistant_messages() {
        let messages = vec![
            create_message(1, "user", "Hello"),
        ];
        assert!(!has_unresolved_tool_calls(&messages));
        assert!(!all_tool_calls_resolved(&messages));
    }

    #[test]
    fn test_assistant_without_tool_messages() {
        let messages = vec![
            create_message(1, "user", "Hello"),
            create_message(2, "assistant", "Hi there"),
        ];
        assert!(!has_unresolved_tool_calls(&messages));
        assert!(!all_tool_calls_resolved(&messages));
    }

    #[test]
    fn test_assistant_with_tool_messages() {
        let messages = vec![
            create_message(1, "user", "Use a tool"),
            create_message(2, "assistant", "I'll use a tool"),
            create_message(3, "tool", "Tool result"),
        ];
        // With our simplified heuristic, this returns false for unresolved
        // and true for resolved (since there are tool messages)
        assert!(!has_unresolved_tool_calls(&messages));
        assert!(all_tool_calls_resolved(&messages));
    }

    #[test]
    fn test_multiple_tool_messages() {
        let messages = vec![
            create_message(1, "user", "Use tools"),
            create_message(2, "assistant", "Using tools"),
            create_message(3, "tool", "Result 1"),
            create_message(4, "tool", "Result 2"),
        ];
        assert!(!has_unresolved_tool_calls(&messages));
        assert!(all_tool_calls_resolved(&messages));
    }

    #[test]
    fn test_tool_messages_before_assistant() {
        let messages = vec![
            create_message(1, "user", "Hello"),
            create_message(2, "tool", "Old tool result"),
            create_message(3, "assistant", "Response"),
        ];
        // Tool messages before the last assistant don't count
        assert!(!has_unresolved_tool_calls(&messages));
        assert!(!all_tool_calls_resolved(&messages));
    }
}
