//! AI completion request handling.
//!
//! This module handles the complete flow of making an AI completion request:
//! 1. Send preRequest event and wait for acknowledgments
//! 2. Process queued messages (move from queue to regular messages)
//! 3. Build AI request from chat messages
//! 4. Make AI completion request
//! 5. Handle response (success or error)

use std::sync::Arc;
use std::time::Duration;

use rhd_ai_client::{AiClient, ChatCompletionRequest, ChatMessage};
use rhd_chat_api::{
    AckCustomEventParams, AddMessageParams, DeleteQueueMessageParams, GetChatParams,
    GetQueueMessagesParams, Message, SendCustomEventParams, UpdateChatParams,
};
use rhd_chat_client::{ChatClient, PluginsMonitor};

use crate::config::PluginConfig;
use crate::trigger_detection::TriggerReason;

/// Handle AI completion request for a chat.
///
/// This function implements the complete AI request flow:
/// 1. Send `ai_completions:preRequest` event
/// 2. Wait for all other plugins to acknowledge
/// 3. Process queued messages if trigger reason is QueuedMessages
/// 4. Acknowledge own event
/// 5. Build and send AI completion request
/// 6. Handle response (add assistant message on success, error tag/message on failure)
pub async fn handle_ai_request(
    client: Arc<ChatClient>,
    plugins_monitor: Arc<PluginsMonitor>,
    ai_client: Arc<AiClient>,
    config: &PluginConfig,
    plugin_id: &str,
    chat_id: i64,
    messages: &[Message],
    trigger_reason: TriggerReason,
    known_version: Option<i64>,
) -> Result<(), AiRequestError> {
    // Send preRequest event
    let event_result = client
        .send_custom_event(SendCustomEventParams {
            event_name: "ai_completions:preRequest".to_string(),
            additional: Some(
                serde_json::to_string(&serde_json::json!({
                    "chatId": chat_id,
                    "triggerReason": match trigger_reason {
                        TriggerReason::QueuedMessages => "queuedMessages",
                        TriggerReason::ToolLoopContinuation => "toolLoopContinuation",
                        TriggerReason::None => "none",
                    }
                }))
                .map_err(|e| AiRequestError::EventSend(e.to_string()))?,
            ),
        })
        .await
        .map_err(|e| AiRequestError::EventSend(e.to_string()))?;

    let event_id = event_result.event_id;

    // Wait for all other plugins to acknowledge
    let except_plugins: Vec<&str> = vec![plugin_id];

    plugins_monitor
        .wait_for_acks_except(&event_id, &except_plugins, Duration::from_secs(30))
        .await
        .map_err(|e| AiRequestError::WaitTimeout(e.to_string()))?;

    // Process queued messages if needed
    let current_messages = if trigger_reason == TriggerReason::QueuedMessages {
        tracing::info!(
            chat_id = chat_id,
            trigger_reason = ?trigger_reason,
            "processing queued messages"
        );
        process_queued_messages(&client, chat_id).await?;
        tracing::info!(chat_id = chat_id, "finished processing queued messages");

        // Refetch messages after processing queued messages using conditional fetch
        let chat_result = client
            .get_chat(GetChatParams {
                chat_id,
                if_version_higher_than: known_version,
            })
            .await
            .map_err(|e| AiRequestError::MessageAdd(e.to_string()))?;
        tracing::info!(
            chat_id = chat_id,
            messages_count = chat_result.messages.len(),
            chat_version = chat_result.chat.version,
            "refetched messages after processing queue"
        );
        chat_result.messages
    } else {
        // Verify state freshness before AI request
        if let Some(version) = known_version {
            tracing::debug!(
                chat_id = chat_id,
                known_version = version,
                "verifying state freshness before AI request"
            );
            let chat_result = client
                .get_chat(GetChatParams {
                    chat_id,
                    if_version_higher_than: Some(version),
                })
                .await
                .map_err(|e| AiRequestError::MessageAdd(e.to_string()))?;
            
            // If we got a response, state was stale and we have fresh data
            if chat_result.chat.version > version {
                tracing::info!(
                    chat_id = chat_id,
                    old_version = version,
                    new_version = chat_result.chat.version,
                    "state was stale, using fresh data"
                );
                chat_result.messages
            } else {
                // State is still fresh, use provided messages
                messages.to_vec()
            }
        } else {
            messages.to_vec()
        }
    };

    // Acknowledge own event
    tracing::debug!(
        chat_id = chat_id,
        event_id = %event_id,
        "acknowledging own event"
    );
    client
        .ack_custom_event(AckCustomEventParams {
            event_id: event_id.clone(),
        })
        .await
        .map_err(|e| AiRequestError::EventAck(e.to_string()))?;

    // Build AI request
    let filtered_messages = filter_messages_for_ai(&current_messages);
    let ai_messages = convert_to_ai_messages(&filtered_messages);

    // Get model config
    let default_model = &config.ai_completions.models["default"];
    let default_name = "default".to_string();
    let model_name = default_model.alias.as_ref().unwrap_or(&default_name);
    let model_config = &config.ai_completions.models[model_name];

    let request = ChatCompletionRequest {
        model: model_config.model.clone().unwrap_or_else(|| "default".to_string()),
        messages: ai_messages,
        tools: None, // TODO: Add tools support
        stream: false,
    };

    // Log the exact request body being sent to the AI provider
    let request_body = serde_json::to_string(&request)
        .map_err(|e| AiRequestError::EventSend(e.to_string()))?;
    tracing::info!(
        chat_id = chat_id,
        request_body = %request_body,
        "sending AI completion request"
    );

    // Make AI completion request
    match ai_client.chat_completion(request).await {
        Ok(response) => {
            // Log the full raw AI provider response
            let response_body = serde_json::to_string(&response)
                .map_err(|e| AiRequestError::EventSend(e.to_string()))?;
            tracing::info!(
                chat_id = chat_id,
                response_body = %response_body,
                "received AI completion response"
            );

            // Add assistant message to chat
            if let Some(choice) = response.choices.first() {
                let content = choice.message.content.clone().unwrap_or_default();

                client
                    .add_message(AddMessageParams {
                        chat_id,
                        role: "assistant".to_string(),
                        content,
                        reasoning_content: None,
                        tags: vec![],
                        is_finished: true,
                        is_streaming: false,
                    })
                    .await
                    .map_err(|e| AiRequestError::MessageAdd(e.to_string()))?;
            }

            Ok(())
        }
        Err(e) => {
            // Log the exact error at ERROR level
            tracing::error!(
                chat_id = chat_id,
                error = %e,
                "AI request failed"
            );

            // Add error tag to chat
            tracing::debug!(chat_id = chat_id, "adding error tag to chat");
            client
                .update_chat(UpdateChatParams {
                    chat_id,
                    title: None,
                    add_tags: vec!["ai_completions:error".to_string()],
                    remove_tags: vec![],
                })
                .await
                .map_err(|e| {
                    tracing::error!(chat_id = chat_id, error = %e, "failed to add error tag");
                    AiRequestError::TagAdd(e.to_string())
                })?;
            tracing::debug!(chat_id = chat_id, "error tag added successfully");

            // Add error message
            let error_content = format!("AI request failed: {}", e);
            tracing::info!(
                chat_id = chat_id,
                role = "assistant",
                content_length = error_content.len(),
                tags = ?vec!["ai_completions:error"],
                "attempting to add error message to chat"
            );
            
            let add_result = client
                .add_message(AddMessageParams {
                    chat_id,
                    role: "assistant".to_string(),
                    content: error_content.clone(),
                    reasoning_content: None,
                    tags: vec!["ai_completions:error".to_string()],
                    is_finished: true,
                    is_streaming: false,
                })
                .await;
            
            match add_result {
                Ok(result) => {
                    tracing::info!(
                        chat_id = chat_id,
                        message_id = result.message_id,
                        "error message added successfully"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %e,
                        "failed to add error message to chat"
                    );
                    return Err(AiRequestError::MessageAdd(e.to_string()));
                }
            }

            Err(AiRequestError::AiRequest(e.to_string()))
        }
    }
}

/// Process queued messages: remove from queue and add as regular messages.
async fn process_queued_messages(
    client: &ChatClient,
    chat_id: i64,
) -> Result<(), AiRequestError> {
    tracing::debug!(chat_id = chat_id, "fetching queued messages");
    // Get queued messages
    let queue_result = client
        .get_queue_messages(GetQueueMessagesParams { chat_id })
        .await
        .map_err(|e| AiRequestError::QueueGet(e.to_string()))?;

    tracing::debug!(
        chat_id = chat_id,
        message_count = queue_result.messages.len(),
        "found queued messages"
    );

    // Delete each queued message and add as regular message
    for queue_msg in queue_result.messages {
        tracing::debug!(
            chat_id = chat_id,
            message_id = queue_msg.id,
            "deleting message from queue"
        );
        // Delete from queue
        client
            .delete_queue_message(DeleteQueueMessageParams {
                message_id: queue_msg.id,
            })
            .await
            .map_err(|e| AiRequestError::QueueDelete(e.to_string()))?;

        tracing::debug!(
            chat_id = chat_id,
            message_id = queue_msg.id,
            role = %queue_msg.role,
            "adding message as regular message"
        );
        // Add as regular message
        client
            .add_message(AddMessageParams {
                chat_id,
                role: queue_msg.role,
                content: queue_msg.content,
                reasoning_content: queue_msg.reasoning_content,
                tags: queue_msg.tags,
                is_finished: true,
                is_streaming: false,
            })
            .await
            .map_err(|e| AiRequestError::MessageAdd(e.to_string()))?;
        tracing::debug!(
            chat_id = chat_id,
            message_id = queue_msg.id,
            "successfully added message"
        );
    }

    Ok(())
}

/// Filter messages for AI request (exclude error messages).
///
/// Only includes messages with roles: user, assistant, system, tool.
/// Excludes messages with role "ai_completions:error".
fn filter_messages_for_ai(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .filter(|m| matches!(m.role.as_str(), "user" | "assistant" | "system" | "tool"))
        .cloned()
        .collect()
}

/// Convert chat messages to AI messages.
///
/// Maps Message objects to ChatMessage enum variants for the AI client.
fn convert_to_ai_messages(messages: &[Message]) -> Vec<ChatMessage> {
    messages
        .iter()
        .map(|m| match m.role.as_str() {
            "user" => ChatMessage::User {
                content: m.content.clone(),
            },
            "assistant" => ChatMessage::Assistant {
                content: Some(m.content.clone()),
                tool_calls: None, // TODO: Add tool calls support
            },
            "system" => ChatMessage::System {
                content: m.content.clone(),
            },
            "tool" => ChatMessage::Tool {
                tool_call_id: String::new(), // TODO: Extract from message
                content: m.content.clone(),
            },
            _ => ChatMessage::User {
                content: m.content.clone(),
            },
        })
        .collect()
}

/// Errors that can occur during AI request handling.
#[derive(Debug, thiserror::Error)]
pub enum AiRequestError {
    #[error("failed to send event: {0}")]
    EventSend(String),
    #[error("timeout waiting for acknowledgments: {0}")]
    WaitTimeout(String),
    #[error("failed to acknowledge event: {0}")]
    EventAck(String),
    #[error("failed to get queued messages: {0}")]
    QueueGet(String),
    #[error("failed to delete queued message: {0}")]
    QueueDelete(String),
    #[error("failed to add message: {0}")]
    MessageAdd(String),
    #[error("failed to add tag: {0}")]
    TagAdd(String),
    #[error("AI request failed: {0}")]
    AiRequest(String),
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
            is_finished: true,
            is_streaming: false,
        }
    }

    #[test]
    fn test_filter_messages_for_ai() {
        let messages = vec![
            create_message(1, "user", "Hello"),
            create_message(2, "ai_completions:error", "Error occurred"),
            create_message(3, "assistant", "Response"),
            create_message(4, "system", "System prompt"),
            create_message(5, "tool", "Tool result"),
        ];

        let filtered = filter_messages_for_ai(&messages);
        assert_eq!(filtered.len(), 4);
        assert_eq!(filtered[0].role, "user");
        assert_eq!(filtered[1].role, "assistant");
        assert_eq!(filtered[2].role, "system");
        assert_eq!(filtered[3].role, "tool");
    }

    #[test]
    fn test_filter_messages_excludes_error_role() {
        let messages = vec![
            create_message(1, "user", "Hello"),
            create_message(2, "ai_completions:error", "Error"),
        ];

        let filtered = filter_messages_for_ai(&messages);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].role, "user");
    }

    #[test]
    fn test_convert_to_ai_messages() {
        let messages = vec![
            create_message(1, "user", "Hello"),
            create_message(2, "assistant", "Hi there"),
            create_message(3, "system", "You are helpful"),
        ];

        let ai_messages = convert_to_ai_messages(&messages);
        assert_eq!(ai_messages.len(), 3);

        match &ai_messages[0] {
            ChatMessage::User { content } => assert_eq!(content, "Hello"),
            _ => panic!("Expected User message"),
        }

        match &ai_messages[1] {
            ChatMessage::Assistant { content, .. } => assert_eq!(content, &Some("Hi there".to_string())),
            _ => panic!("Expected Assistant message"),
        }

        match &ai_messages[2] {
            ChatMessage::System { content } => assert_eq!(content, "You are helpful"),
            _ => panic!("Expected System message"),
        }
    }
}
