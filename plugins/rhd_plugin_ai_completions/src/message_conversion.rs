//! Chat history → AI request conversion with integrity validation.
//!
//! Core principle: **a request is either complete or it is not sent.** This module
//! renders the stored chat history faithfully:
//!
//! - assistant `tool_calls` are forwarded field-for-field (D2); per-call `tags` are
//!   RHD-internal orchestration metadata and never reach the provider (the AI-client
//!   `ToolCall` type deliberately has no `tags` field),
//! - assistant `reasoning_content` is forwarded when present and enabled per model (D3),
//! - `tool` messages carry their `tool_call_id` (D1),
//! - unknown roles and `ai_completions:error`-tagged messages are the only permitted
//!   exclusions (D5),
//! - any inconsistency in the tool-call sequence aborts the whole build (D4). The
//!   caller then parks the chat instead of sending a lossy request.

use std::collections::HashSet;

use rhd_ai_client::{ChatMessage, FunctionCall as AiFunctionCall, ToolCall as AiToolCall};
use rhd_chat_api::Message;

use crate::config::ModelConfig;

/// Tag marking internal bookkeeping messages; never sent to the provider.
const ERROR_TAG: &str = "ai_completions:error";

/// Reasons `build_chat_messages` refuses to produce a request.
#[derive(Debug, thiserror::Error)]
pub enum ConversionError {
    #[error("assistant message {message_id} tool call {tool_call_id} has no matching tool result")]
    UnresolvedToolCall { message_id: i64, tool_call_id: String },
    #[error("tool message {message_id} references undeclared tool call {tool_call_id}")]
    OrphanToolResult { message_id: i64, tool_call_id: String },
    #[error("tool message {message_id} has no tool_call_id")]
    MissingToolCallId { message_id: i64 },
    #[error("assistant message {message_id} has no content, tool calls, or reasoning")]
    EmptyAssistantMessage { message_id: i64 },
}

/// Build the provider request messages from stored chat history.
///
/// `Err` means the history is inconsistent (the D4 trigger gate was bypassed or raced,
/// e.g. another plugin deleted a message between the decision and the build). The caller
/// must send nothing.
pub fn build_chat_messages(
    messages: &[Message],
    model_config: &ModelConfig,
) -> Result<Vec<ChatMessage>, ConversionError> {
    let included: Vec<&Message> = messages.iter().filter(|m| is_included(m)).collect();
    validate_tool_call_integrity(&included)?;
    Ok(included
        .into_iter()
        .filter_map(|m| to_chat_message(m, model_config))
        .collect())
}

/// Policy inclusion (D5): a known role and not an error-tagged bookkeeping message.
fn is_included(message: &Message) -> bool {
    matches!(
        message.role.as_str(),
        "user" | "assistant" | "system" | "tool"
    ) && !message.tags.iter().any(|tag| tag == ERROR_TAG)
}

/// Validate the OpenAI tool-call contract over the included sequence:
/// every declared id is answered before the next assistant turn, every tool message
/// carries an id and references a declared one, and no assistant message is empty.
fn validate_tool_call_integrity(messages: &[&Message]) -> Result<(), ConversionError> {
    let mut declared: HashSet<String> = HashSet::new();
    // (id of the declaring assistant message, tool call id) awaiting a result.
    let mut pending: Vec<(i64, String)> = Vec::new();

    for message in messages {
        match message.role.as_str() {
            "assistant" => {
                if let Some((owner_id, call_id)) = pending.first() {
                    return Err(ConversionError::UnresolvedToolCall {
                        message_id: *owner_id,
                        tool_call_id: call_id.clone(),
                    });
                }
                let has_reasoning = message
                    .reasoning_content
                    .as_deref()
                    .is_some_and(|reasoning| !reasoning.is_empty());
                if message.content.is_empty() && message.tool_calls.is_empty() && !has_reasoning {
                    return Err(ConversionError::EmptyAssistantMessage {
                        message_id: message.id,
                    });
                }
                for tool_call in &message.tool_calls {
                    declared.insert(tool_call.id.clone());
                    pending.push((message.id, tool_call.id.clone()));
                }
            }
            "tool" => {
                let Some(call_id) = message.tool_call_id.as_deref() else {
                    return Err(ConversionError::MissingToolCallId {
                        message_id: message.id,
                    });
                };
                if !declared.contains(call_id) {
                    return Err(ConversionError::OrphanToolResult {
                        message_id: message.id,
                        tool_call_id: call_id.to_string(),
                    });
                }
                pending.retain(|(_, pending_id)| pending_id != call_id);
            }
            _ => {}
        }
    }

    if let Some((owner_id, call_id)) = pending.first() {
        return Err(ConversionError::UnresolvedToolCall {
            message_id: *owner_id,
            tool_call_id: call_id.clone(),
        });
    }
    Ok(())
}

/// Convert one stored message to its provider representation.
///
/// Returns `None` only for policy-excluded messages (D5). It never strips tool-call
/// data: a `tool` message without an id cannot reach the mapping stage because
/// `validate_tool_call_integrity` runs first and rejects it.
pub fn to_chat_message(message: &Message, model_config: &ModelConfig) -> Option<ChatMessage> {
    if !is_included(message) {
        return None;
    }
    match message.role.as_str() {
        "user" => Some(ChatMessage::User {
            content: message.content.clone(),
        }),
        "system" => Some(ChatMessage::System {
            content: message.content.clone(),
        }),
        "assistant" => {
            let tool_calls = if message.tool_calls.is_empty() {
                None
            } else {
                Some(
                    message
                        .tool_calls
                        .iter()
                        .map(|tool_call| AiToolCall {
                            id: tool_call.id.clone(),
                            call_type: tool_call.call_type.clone(),
                            function: AiFunctionCall {
                                name: tool_call.function.name.clone(),
                                arguments: tool_call.function.arguments.clone(),
                            },
                        })
                        .collect(),
                )
            };
            let reasoning_content = if model_config.sends_reasoning_content() {
                message
                    .reasoning_content
                    .clone()
                    .filter(|reasoning| !reasoning.is_empty())
            } else {
                None
            };
            Some(ChatMessage::Assistant {
                content: Some(message.content.clone()),
                tool_calls,
                reasoning_content,
            })
        }
        "tool" => Some(ChatMessage::Tool {
            tool_call_id: message.tool_call_id.clone().unwrap_or_default(),
            content: message.content.clone(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rhd_chat_api::{FunctionCall, ToolCall};

    fn model_config(send_reasoning: Option<bool>) -> ModelConfig {
        ModelConfig {
            alias: None,
            base_url: None,
            api_key: None,
            model: None,
            send_reasoning_content: send_reasoning,
        }
    }

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
                    arguments: format!("{{\"city\":\"{call_id}\"}}"),
                },
                tags: vec!["pending".to_string()],
            })
            .collect();
        msg
    }

    fn tool_result(id: i64, tool_call_id: &str) -> Message {
        let mut msg = message(id, "tool", "result");
        msg.tool_call_id = Some(tool_call_id.to_string());
        msg
    }

    fn default_config() -> ModelConfig {
        model_config(None)
    }

    #[test]
    fn assistant_tool_calls_are_forwarded_verbatim() {
        let messages = vec![
            message(1, "user", "Weather in Paris and London?"),
            assistant_with_calls(2, &["call_a", "call_b"]),
            tool_result(3, "call_a"),
            tool_result(4, "call_b"),
        ];
        let built = build_chat_messages(&messages, &default_config()).unwrap();

        match &built[1] {
            ChatMessage::Assistant {
                content, tool_calls, ..
            } => {
                assert_eq!(content, &Some(String::new()));
                let calls = tool_calls.as_ref().expect("tool calls forwarded");
                assert_eq!(calls.len(), 2);
                assert_eq!(calls[0].id, "call_a");
                assert_eq!(calls[0].call_type, "function");
                assert_eq!(calls[0].function.name, "get_weather");
                assert_eq!(calls[0].function.arguments, "{\"city\":\"call_a\"}");
                assert_eq!(calls[1].id, "call_b");
            }
            other => panic!("expected assistant, got {other:?}"),
        }
        // Per-call-tags are RHD-internal and must not reach the provider.
        let json = serde_json::to_string(&built).unwrap();
        assert!(!json.contains("pending"));
        assert!(!json.contains("\"tags\""));
    }

    #[test]
    fn tool_messages_carry_their_tool_call_id() {
        let messages = vec![
            assistant_with_calls(1, &["call_a"]),
            tool_result(2, "call_a"),
        ];
        let built = build_chat_messages(&messages, &default_config()).unwrap();
        match &built[1] {
            ChatMessage::Tool { tool_call_id, content } => {
                assert_eq!(tool_call_id, "call_a");
                assert_eq!(content, "result");
            }
            other => panic!("expected tool, got {other:?}"),
        }
    }

    #[test]
    fn reasoning_content_sent_when_enabled_and_present() {
        let mut assistant = message(2, "assistant", "answer");
        assistant.reasoning_content = Some("deep thought".to_string());
        let messages = vec![message(1, "user", "q"), assistant];
        let built = build_chat_messages(&messages, &model_config(Some(true))).unwrap();
        match &built[1] {
            ChatMessage::Assistant { reasoning_content, .. } => {
                assert_eq!(reasoning_content, &Some("deep thought".to_string()));
            }
            other => panic!("expected assistant, got {other:?}"),
        }
    }

    #[test]
    fn reasoning_content_suppressed_when_disabled() {
        let mut assistant = message(2, "assistant", "answer");
        assistant.reasoning_content = Some("deep thought".to_string());
        let messages = vec![message(1, "user", "q"), assistant];
        let built = build_chat_messages(&messages, &model_config(Some(false))).unwrap();
        match &built[1] {
            ChatMessage::Assistant { reasoning_content, .. } => {
                assert_eq!(reasoning_content, &None);
            }
            other => panic!("expected assistant, got {other:?}"),
        }
        let json = serde_json::to_string(&built).unwrap();
        assert!(!json.contains("reasoning_content"));
    }

    #[test]
    fn empty_reasoning_content_is_skipped() {
        let mut assistant = message(2, "assistant", "answer");
        assistant.reasoning_content = Some(String::new());
        let messages = vec![message(1, "user", "q"), assistant];
        let built = build_chat_messages(&messages, &default_config()).unwrap();
        match &built[1] {
            ChatMessage::Assistant { reasoning_content, .. } => {
                assert_eq!(reasoning_content, &None);
            }
            other => panic!("expected assistant, got {other:?}"),
        }
    }

    #[test]
    fn unknown_role_is_excluded_not_coerced() {
        let messages = vec![
            message(1, "user", "Hello"),
            message(2, "ai_completions:something", "noise"),
        ];
        let built = build_chat_messages(&messages, &default_config()).unwrap();
        assert_eq!(built.len(), 1);
        assert!(matches!(built[0], ChatMessage::User { .. }));
    }

    #[test]
    fn error_tagged_assistant_is_excluded() {
        // The latent bug this fixes: error text used to be replayed as a normal
        // assistant turn because filtering was by role, not tag.
        let mut broken = message(2, "assistant", "AI request failed: timeout");
        broken.tags = vec!["ai_completions:error".to_string()];
        let messages = vec![message(1, "user", "Hello"), broken];
        let built = build_chat_messages(&messages, &default_config()).unwrap();
        assert_eq!(built.len(), 1);
    }

    #[test]
    fn unresolved_tool_call_aborts_build() {
        let messages = vec![
            message(1, "user", "Use tools"),
            assistant_with_calls(2, &["call_a", "call_b"]),
            tool_result(3, "call_a"),
        ];
        let err = build_chat_messages(&messages, &default_config()).unwrap_err();
        match err {
            ConversionError::UnresolvedToolCall { message_id, tool_call_id } => {
                assert_eq!(message_id, 2);
                assert_eq!(tool_call_id, "call_b");
            }
            other => panic!("expected UnresolvedToolCall, got {other:?}"),
        }
    }

    #[test]
    fn orphan_tool_result_aborts_build() {
        let messages = vec![
            message(1, "user", "hi"),
            tool_result(2, "call_never_declared"),
        ];
        let err = build_chat_messages(&messages, &default_config()).unwrap_err();
        match err {
            ConversionError::OrphanToolResult { message_id, tool_call_id } => {
                assert_eq!(message_id, 2);
                assert_eq!(tool_call_id, "call_never_declared");
            }
            other => panic!("expected OrphanToolResult, got {other:?}"),
        }
    }

    #[test]
    fn missing_tool_call_id_aborts_build() {
        let messages = vec![message(1, "tool", "result without id")];
        let err = build_chat_messages(&messages, &default_config()).unwrap_err();
        assert!(matches!(
            err,
            ConversionError::MissingToolCallId { message_id: 1 }
        ));
    }

    #[test]
    fn empty_assistant_message_aborts_build() {
        let messages = vec![message(1, "user", "hi"), message(2, "assistant", "")];
        let err = build_chat_messages(&messages, &default_config()).unwrap_err();
        assert!(matches!(
            err,
            ConversionError::EmptyAssistantMessage { message_id: 2 }
        ));
    }

    #[test]
    fn assistant_with_only_tool_calls_is_not_empty() {
        let messages = vec![
            assistant_with_calls(1, &["call_a"]),
            tool_result(2, "call_a"),
        ];
        assert!(build_chat_messages(&messages, &default_config()).is_ok());
    }

    #[test]
    fn consistent_history_never_errors() {
        let messages = vec![
            message(1, "system", "You are helpful"),
            message(2, "user", "Weather?"),
            assistant_with_calls(3, &["call_a", "call_b"]),
            tool_result(4, "call_a"),
            tool_result(5, "call_b"),
            message(6, "assistant", "Sunny and mild"),
        ];
        let built = build_chat_messages(&messages, &default_config()).unwrap();
        assert_eq!(built.len(), 6);
    }
}
