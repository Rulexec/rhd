//! AI completion request handling.
//!
//! This module handles the complete flow of making an AI completion request:
//! 1. Send preRequest event and wait for acknowledgments
//! 2. Process queued messages (move from queue to regular messages)
//! 3. Build and validate AI request from chat messages (D4: refuse inconsistent histories)
//! 4. Make AI completion request
//! 5. Handle response (success or error)

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use rhd_ai_client::{AiClient, ChatCompletionRequest};
use rhd_chat_api::{
    AckCustomEventParams, AddMessageParams, GetChatParams, Message, SendCustomEventParams,
    StreamFinishParams, StreamPushParams, StreamToolCallDelta, UpdateChatParams,
    UpdateMessageParams,
};
use rhd_chat_client::{ChatClient, PluginsMonitor};

use crate::config::PluginConfig;
use crate::message_conversion;
use crate::queued_messages::process_queued_messages;
use crate::trigger_detection::TriggerReason;

/// Handle AI completion request for a chat.
///
/// This function implements the complete AI request flow:
/// 1. Send `ai_completions:preRequest` event
/// 2. Wait for all other plugins to acknowledge
/// 3. Process queued messages if trigger reason is QueuedMessages
/// 4. Acknowledge own event
/// 5. Build and validate the AI completion request — on validation failure, park the chat and send nothing (D4)
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
            chat_id: None,
            message_id: None,
            tool_call_id: None,
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
            is_rejected: None,
        })
        .await
        .map_err(|e| AiRequestError::EventAck(e.to_string()))?;

    // Get model config
    let default_model = &config.ai_completions.models["default"];
    let default_name = "default".to_string();
    let model_name = default_model.alias.as_ref().unwrap_or(&default_name);
    let model_config = &config.ai_completions.models[model_name];

    // Build AI request. A request is either complete or it is not sent (D4): an
    // inconsistent history parks the chat instead of producing a stripped request.
    let ai_messages = match message_conversion::build_chat_messages(&current_messages, model_config) {
        Ok(built) => built,
        Err(error) => {
            tracing::error!(
                chat_id = chat_id,
                error = %error,
                "message conversion failed; refusing to send request, parking chat"
            );
            client
                .update_chat(UpdateChatParams {
                    chat_id,
                    title: None,
                    add_tags: vec!["ai_completions:error".to_string()],
                    remove_tags: vec![],
                })
                .await
                .map_err(|tag_error| {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %tag_error,
                        "failed to park chat after conversion error"
                    );
                    AiRequestError::TagAdd(tag_error.to_string())
                })?;
            return Err(AiRequestError::MessageConversion(error.to_string()));
        }
    };

    // Create message with streaming flags BEFORE AI request
    let add_result = client
        .add_message(AddMessageParams {
            chat_id,
            role: "assistant".to_string(),
            content: String::new(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
            is_finished: false,
            is_streaming: true,
        })
        .await
        .map_err(|e| AiRequestError::MessageAdd(e.to_string()))?;

    let message_id = add_result.message_id;
    tracing::info!(chat_id = chat_id, message_id = message_id, "created streaming message");

    // Build streaming AI request
    let request = ChatCompletionRequest {
        model: model_config.model.clone().unwrap_or_else(|| "default".to_string()),
        messages: ai_messages,
        tools: None, // TODO: Add tools support
        stream: true,
    };

    // Log the exact request body being sent to the AI provider
    let request_body = serde_json::to_string(&request)
        .map_err(|e| AiRequestError::EventSend(e.to_string()))?;
    tracing::info!(
        chat_id = chat_id,
        request_body = %request_body,
        "sending streaming AI request"
    );

    // Accumulate final content
    let mut final_reasoning = String::new();
    let mut final_content = String::new();
    let mut final_tool_calls: Vec<StreamToolCallDelta> = Vec::new();

    // Make streaming AI request
    match ai_client.chat_completion_stream(request).await {
        Ok(mut stream) => {
            // Process each chunk from the stream
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        // Reasoning content delta
                        if let Some(reasoning) = &chunk.reasoning_content {
                            if !reasoning.is_empty() {
                                final_reasoning.push_str(reasoning);
                                client
                                    .stream_push(StreamPushParams {
                                        chat_id,
                                        reasoning_content: Some(reasoning.clone()),
                                        content: None,
                                        tool_calls: None,
                                    })
                                    .await
                                    .map_err(|e| {
                                        tracing::error!(chat_id = chat_id, error = %e, "failed to push reasoning delta");
                                        AiRequestError::StreamPush(e.to_string())
                                    })?;
                            }
                        }

                        // Content delta
                        if let Some(content) = &chunk.content {
                            if !content.is_empty() {
                                final_content.push_str(content);
                                client
                                    .stream_push(StreamPushParams {
                                        chat_id,
                                        reasoning_content: None,
                                        content: Some(content.clone()),
                                        tool_calls: None,
                                    })
                                    .await
                                    .map_err(|e| {
                                        tracing::error!(chat_id = chat_id, error = %e, "failed to push content delta");
                                        AiRequestError::StreamPush(e.to_string())
                                    })?;
                            }
                        }

                        // Tool call deltas
                        if let Some(tool_calls) = &chunk.tool_calls {
                            let mut tool_deltas = Vec::new();
                            for tc in tool_calls {
                                let id = tc.id.clone().unwrap_or_default();
                                let name = tc.function.as_ref().and_then(|f| f.name.clone()).unwrap_or_default();
                                let arguments = tc.function.as_ref().and_then(|f| f.arguments.clone()).unwrap_or_default();

                                // Merge with existing tool call by ID
                                if let Some(existing) = final_tool_calls.iter_mut().find(|t| t.id == id) {
                                    existing.arguments.push_str(&arguments);
                                } else {
                                    final_tool_calls.push(StreamToolCallDelta {
                                        id: id.clone(),
                                        name,
                                        arguments,
                                    });
                                }

                                tool_deltas.push(StreamToolCallDelta {
                                    id,
                                    name: tc.function.as_ref().and_then(|f| f.name.clone()).unwrap_or_default(),
                                    arguments: tc.function.as_ref().and_then(|f| f.arguments.clone()).unwrap_or_default(),
                                });
                            }

                            if !tool_deltas.is_empty() {
                                client
                                    .stream_push(StreamPushParams {
                                        chat_id,
                                        reasoning_content: None,
                                        content: None,
                                        tool_calls: Some(tool_deltas),
                                    })
                                    .await
                                    .map_err(|e| {
                                        tracing::error!(chat_id = chat_id, error = %e, "failed to push tool call delta");
                                        AiRequestError::StreamPush(e.to_string())
                                    })?;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(chat_id = chat_id, error = %e, "error during streaming");
                        // Finish stream and update message with error
                        let _ = client
                            .stream_finish(StreamFinishParams {
                                chat_id,
                                reasoning_content: None,
                                content: None,
                                tool_calls: None,
                            })
                            .await;

                        let error_content = format!("AI streaming error: {}", e);
                        client
                            .update_message(UpdateMessageParams {
                                message_id,
                                content: Some(error_content.clone()),
                                reasoning_content: None,
                                role: None,
                                add_tags: vec!["ai_completions:error".to_string()],
                                remove_tags: vec![],
                                is_finished: Some(true),
                                is_streaming: Some(false),
                                tool_calls: None,
                            })
                            .await
                            .map_err(|e| AiRequestError::MessageUpdate(e.to_string()))?;

                        return Err(AiRequestError::AiRequest(e.to_string()));
                    }
                }
            }

            // Stream completed successfully
            tracing::info!(
                chat_id = chat_id,
                message_id = message_id,
                content_length = final_content.len(),
                reasoning_length = final_reasoning.len(),
                tool_calls_count = final_tool_calls.len(),
                "streaming completed successfully"
            );

            // Finish the stream
            client
                .stream_finish(StreamFinishParams {
                    chat_id,
                    reasoning_content: if final_reasoning.is_empty() { None } else { Some(final_reasoning.clone()) },
                    content: if final_content.is_empty() { None } else { Some(final_content.clone()) },
                    tool_calls: if final_tool_calls.is_empty() { None } else { Some(final_tool_calls.clone()) },
                })
                .await
                .map_err(|e| {
                    tracing::error!(chat_id = chat_id, error = %e, "failed to finish stream");
                    AiRequestError::StreamFinish(e.to_string())
                })?;

            // Update message with final content
            let tool_calls_json = if final_tool_calls.is_empty() {
                None
            } else {
                // Convert StreamToolCallDelta to ToolCall format for storage
                let tool_calls: Vec<rhd_chat_api::ToolCall> = final_tool_calls
                    .iter()
                    .map(|tc| rhd_chat_api::ToolCall {
                        id: tc.id.clone(),
                        call_type: "function".to_string(),
                        function: rhd_chat_api::FunctionCall {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                        },
                        tags: vec![],
                    })
                    .collect();
                Some(serde_json::to_string(&tool_calls).unwrap_or_default())
            };

            client
                .update_message(UpdateMessageParams {
                    message_id,
                    content: Some(final_content),
                    reasoning_content: if final_reasoning.is_empty() { None } else { Some(final_reasoning) },
                    role: None,
                    add_tags: vec![],
                    remove_tags: vec![],
                    is_finished: Some(true),
                    is_streaming: Some(false),
                    tool_calls: tool_calls_json,
                })
                .await
                .map_err(|e| {
                    tracing::error!(chat_id = chat_id, error = %e, "failed to update message after streaming");
                    AiRequestError::MessageUpdate(e.to_string())
                })?;

            Ok(())
        }
        Err(e) => {
            // AI request failed before streaming started
            tracing::error!(chat_id = chat_id, error = %e, "AI request failed");

            // Finish the stream
            let _ = client
                .stream_finish(StreamFinishParams {
                    chat_id,
                    reasoning_content: None,
                    content: None,
                    tool_calls: None,
                })
                .await;

            // Add error tag to chat
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

            // Update message with error content
            let error_content = format!("AI request failed: {}", e);
            client
                .update_message(UpdateMessageParams {
                    message_id,
                    content: Some(error_content),
                    reasoning_content: None,
                    role: None,
                    add_tags: vec!["ai_completions:error".to_string()],
                    remove_tags: vec![],
                    is_finished: Some(true),
                    is_streaming: Some(false),
                    tool_calls: None,
                })
                .await
                .map_err(|e| AiRequestError::MessageUpdate(e.to_string()))?;

            Err(AiRequestError::AiRequest(e.to_string()))
        }
    }
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
    #[error("failed to update message: {0}")]
    MessageUpdate(String),
    #[error("failed to add tag: {0}")]
    TagAdd(String),
    #[error("failed to push to stream: {0}")]
    StreamPush(String),
    #[error("failed to finish stream: {0}")]
    StreamFinish(String),
    #[error("AI request failed: {0}")]
    AiRequest(String),
    #[error("message conversion failed: {0}")]
    MessageConversion(String),
}
