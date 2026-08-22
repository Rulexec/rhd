# Phase 4: AI Completions Plugin Implementation

## Overview

This phase implements the core logic of the `rhd_plugin_ai_completions` plugin, including chat monitoring, AI request handling, tool resolution detection, and error handling.

**Scope**:
- Implement chat monitoring with trigger condition detection
- Implement AI request flow with preRequest event coordination
- Implement tool call resolution detection
- Implement error handling with tags and messages
- Integrate all components in plugin.rs

**Out of Scope**:
- Integration testing (Phase 5)
- Server-side enhancements (Phase 2)
- Chat client enhancements (Phase 3)

## Files to Create

### 1. `plugins/rhd_plugin_ai_completions/src/tool_resolution.rs` (CREATE)

**Purpose**: Detect when tool calls are resolved.

**Complete Implementation**:
```rust
use rhd_chat_api::Message;

/// Check if there are unresolved tool calls in the message history.
/// 
/// Returns true if the last assistant message has tool calls that don't have
/// corresponding tool result messages.
pub fn has_unresolved_tool_calls(messages: &[Message]) -> bool {
    // Find the last assistant message with tool calls
    let last_assistant_with_tools = messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant" && has_tool_calls(m));
    
    let last_assistant = match last_assistant_with_tools {
        Some(m) => m,
        None => return false, // No assistant messages with tool calls
    };
    
    // Extract tool call IDs from the last assistant message
    let tool_call_ids = extract_tool_call_ids(last_assistant);
    
    // Count tool result messages after the last assistant message
    let last_assistant_idx = messages.iter().position(|m| m.id == last_assistant.id).unwrap();
    let tool_results_count = messages[last_assistant_idx + 1..]
        .iter()
        .filter(|m| m.role == "tool")
        .count();
    
    // Check if all tool calls have results
    tool_results_count < tool_call_ids.len()
}

/// Check if all tool calls from the last assistant message are resolved.
/// 
/// Returns true if the last assistant message has tool calls and all of them
/// have corresponding tool result messages.
pub fn all_tool_calls_resolved(messages: &[Message]) -> bool {
    // Find the last assistant message with tool calls
    let last_assistant_with_tools = messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant" && has_tool_calls(m));
    
    let last_assistant = match last_assistant_with_tools {
        Some(m) => m,
        None => return false, // No assistant messages with tool calls
    };
    
    // Extract tool call IDs
    let tool_call_ids = extract_tool_call_ids(last_assistant);
    if tool_call_ids.is_empty() {
        return false;
    }
    
    // Count tool result messages after the last assistant message
    let last_assistant_idx = messages.iter().position(|m| m.id == last_assistant.id).unwrap();
    let tool_results_count = messages[last_assistant_idx + 1..]
        .iter()
        .filter(|m| m.role == "tool")
        .count();
    
    // Check if all tool calls have results
    tool_results_count >= tool_call_ids.len()
}

/// Check if a message has tool calls.
fn has_tool_calls(message: &Message) -> bool {
    // Message doesn't have tool_calls field in the API type
    // We need to check if the content indicates tool calls
    // For now, we'll check if there are tool messages after this assistant message
    // This is a simplified check - in production, you'd parse the message content
    // or add tool_calls field to the Message struct
    false // Placeholder - needs actual implementation based on message structure
}

/// Extract tool call IDs from a message.
fn extract_tool_call_ids(message: &Message) -> Vec<String> {
    // Placeholder - needs actual implementation based on message structure
    vec![]
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
    fn test_no_tool_calls() {
        let messages = vec![
            create_message(1, "user", "Hello"),
            create_message(2, "assistant", "Hi there"),
        ];
        assert!(!has_unresolved_tool_calls(&messages));
        assert!(!all_tool_calls_resolved(&messages));
    }
}
```

**Note**: The `has_tool_calls` and `extract_tool_call_ids` functions need to be implemented based on how tool calls are represented in messages. This may require:
1. Adding a `tool_calls` field to the `Message` struct in `rhd_chat_api`
2. Parsing tool calls from message content
3. Using a different detection mechanism

For now, these are placeholders. The actual implementation will depend on how the system represents tool calls in messages.

### 2. `plugins/rhd_plugin_ai_completions/src/trigger_detection.rs` (CREATE)

**Purpose**: Detect trigger conditions for AI completions using ChatMonitor from rhd_chat_client.

**Note**: The ChatMonitor is now provided by rhd_chat_client (Phase 3). This module only contains trigger detection logic.

**Complete Implementation**:
```rust
use rhd_chat_client::ChatState;

use crate::tool_resolution;

/// Reason for triggering AI completion.
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerReason {
    /// Queued messages present, no unresolved tool calls
    QueuedMessages,
    /// Tool loop continuation: last assistant has tool calls, all resolved
    ToolLoopContinuation,
    /// No trigger needed
    None,
}

/// Check if chat should trigger AI completion.
pub fn should_trigger(chat_state: &ChatState) -> TriggerReason {
    // Skip if chat has error tag
    if has_error_tag(chat_state) {
        return TriggerReason::None;
    }

    // Check for queued messages mode
    if chat_state.queued_messages_count > 0 && !tool_resolution::has_unresolved_tool_calls(&chat_state.messages) {
        return TriggerReason::QueuedMessages;
    }

    // Check for tool loop continuation mode
    if tool_resolution::all_tool_calls_resolved(&chat_state.messages) {
        return TriggerReason::ToolLoopContinuation;
    }

    TriggerReason::None
}

/// Check if chat has error tag.
pub fn has_error_tag(chat_state: &ChatState) -> bool {
    chat_state.tags.contains(&"ai_completions:error".to_string())
}
```

### 3. `plugins/rhd_plugin_ai_completions/src/ai_request.rs` (CREATE)

**Purpose**: Handle AI completion requests.

**Complete Implementation**:
```rust
use std::sync::Arc;
use std::time::Duration;

use rhd_ai_client::{AiClient, ChatCompletionRequest, ChatMessage};
use rhd_chat_api::{
    AddMessageParams, AddQueueMessageParams, DeleteQueueMessageParams, GetQueueMessagesParams,
    Message, SendCustomEventParams, UpdateChatParams,
};
use rhd_chat_client::{ChatClient, PluginsMonitor};

use crate::trigger_detection::TriggerReason;
use crate::config::PluginConfig;

/// Handle AI completion request for a chat.
pub async fn handle_ai_request(
    client: Arc<ChatClient>,
    plugins_monitor: Arc<PluginsMonitor>,
    ai_client: Arc<AiClient>,
    config: &PluginConfig,
    plugin_id: &str,
    chat_id: i64,
    messages: &[Message],
    trigger_reason: TriggerReason,
) -> Result<(), AiRequestError> {
    // Send preRequest event
    let event_result = client
        .send_custom_event(SendCustomEventParams {
            event_name: "ai_completions:preRequest".to_string(),
            additional: Some(serde_json::to_string(&serde_json::json!({
                "chatId": chat_id,
                "triggerReason": match trigger_reason {
                    TriggerReason::QueuedMessages => "queuedMessages",
                    TriggerReason::ToolLoopContinuation => "toolLoopContinuation",
                    TriggerReason::None => "none",
                }
            })).unwrap()),
        })
        .await
        .map_err(|e| AiRequestError::EventSend(e.to_string()))?;

    let event_id = event_result.event_id;

    // Wait for all other plugins to acknowledge
    let active_plugins = plugins_monitor.get_plugin_ids().await;
    let except_plugins: Vec<&str> = vec![plugin_id];
    
    plugins_monitor
        .wait_for_acks_except(&event_id, &except_plugins, Duration::from_secs(30))
        .await
        .map_err(|e| AiRequestError::WaitTimeout(e.to_string()))?;

    // Process queued messages if needed
    if trigger_reason == TriggerReason::QueuedMessages {
        process_queued_messages(&client, chat_id).await?;
    }

    // Acknowledge own event
    client
        .ack_custom_event(rhd_chat_api::AckCustomEventParams {
            event_id: event_id.clone(),
        })
        .await
        .map_err(|e| AiRequestError::EventAck(e.to_string()))?;

    // Build AI request
    let filtered_messages = filter_messages_for_ai(messages);
    let ai_messages = convert_to_ai_messages(&filtered_messages);
    
    // Get model config
    let default_model = &config.ai_completions.models["default"];
    let model_name = default_model.alias.as_ref().unwrap_or(&"default".to_string());
    let model_config = &config.ai_completions.models[model_name];
    
    let request = ChatCompletionRequest {
        model: model_config.model.clone(),
        messages: ai_messages,
        tools: None, // TODO: Add tools support
        stream: false,
    };

    // Make AI completion request
    match ai_client.chat_completion(request).await {
        Ok(response) => {
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
                    })
                    .await
                    .map_err(|e| AiRequestError::MessageAdd(e.to_string()))?;
            }
            
            Ok(())
        }
        Err(e) => {
            // Add error tag to chat
            client
                .update_chat(UpdateChatParams {
                    chat_id,
                    title: None,
                    add_tags: vec!["ai_completions:error".to_string()],
                    remove_tags: vec![],
                })
                .await
                .map_err(|e| AiRequestError::TagAdd(e.to_string()))?;

            // Add error message
            let error_content = format!("AI request failed: {}", e);
            client
                .add_message(AddMessageParams {
                    chat_id,
                    role: "ai_completions:error".to_string(),
                    content: error_content,
                    reasoning_content: None,
                    tags: vec![],
                })
                .await
                .map_err(|e| AiRequestError::MessageAdd(e.to_string()))?;

            Err(AiRequestError::AiRequest(e.to_string()))
        }
    }
}

/// Process queued messages: remove from queue and add as regular messages.
async fn process_queued_messages(
    client: &ChatClient,
    chat_id: i64,
) -> Result<(), AiRequestError> {
    // Get queued messages
    let queue_result = client
        .get_queue_messages(GetQueueMessagesParams { chat_id })
        .await
        .map_err(|e| AiRequestError::QueueGet(e.to_string()))?;

    // Delete each queued message and add as regular message
    for queue_msg in queue_result.messages {
        // Delete from queue
        client
            .delete_queue_message(DeleteQueueMessageParams {
                message_id: queue_msg.id,
            })
            .await
            .map_err(|e| AiRequestError::QueueDelete(e.to_string()))?;

        // Add as regular message
        client
            .add_message(AddMessageParams {
                chat_id,
                role: queue_msg.role,
                content: queue_msg.content,
                reasoning_content: queue_msg.reasoning_content,
                tags: queue_msg.tags,
            })
            .await
            .map_err(|e| AiRequestError::MessageAdd(e.to_string()))?;
    }

    Ok(())
}

/// Filter messages for AI request (exclude error messages).
fn filter_messages_for_ai(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .filter(|m| matches!(m.role.as_str(), "user" | "assistant" | "system" | "tool"))
        .cloned()
        .collect()
}

/// Convert chat messages to AI messages.
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
```

### 4. `plugins/rhd_plugin_ai_completions/src/plugin.rs` (MODIFY)

**Purpose**: Integrate all components and implement main plugin loop.

**Replace the stub implementation** with:
```rust
use std::sync::Arc;
use std::time::Duration;

use rhd_ai_client::AiClient;
use rhd_chat_api::RegisterPluginParams;
use rhd_chat_client::{ChatClient, PluginsMonitor};

use crate::ai_request;
use crate::trigger_detection;
use rhd_chat_client::ChatMonitor;
use crate::config::{self, PluginConfig};

pub async fn run_plugin(
    server_url: &str,
    plugin_id: &str,
    config: PluginConfig,
) -> Result<(), PluginError> {
    // Connect to chat server
    let client = Arc::new(
        ChatClient::connect(server_url)
            .await
            .map_err(|e| PluginError::Connection(e.to_string()))?,
    );

    tracing::info!("Connected to chat server");

    // Register as plugin
    client
        .register_plugin(RegisterPluginParams {
            plugin_id: plugin_id.to_string(),
        })
        .await
        .map_err(|e| PluginError::Registration(e.to_string()))?;

    tracing::info!("Registered as plugin: {}", plugin_id);

    // Get pending acks
    let pending_acks = client
        .get_pending_acks(rhd_chat_api::GetPendingAcksParams {})
        .await
        .map_err(|e| PluginError::PendingAcks(e.to_string()))?;

    tracing::info!("Found {} pending acks", pending_acks.pending_events.len());

    // Process pending acks
    for event in pending_acks.pending_events {
        tracing::info!("Processing pending event: {}", event.event_name);
        // TODO: Implement pending ack processing based on event type
        // For now, just acknowledge them
        client
            .ack_custom_event(rhd_chat_api::AckCustomEventParams {
                event_id: event.event_id,
            })
            .await
            .map_err(|e| PluginError::PendingAcks(e.to_string()))?;
    }

    // Create plugins monitor
    let plugins_monitor = Arc::new(
        client
            .create_plugins_monitor()
            .await
            .map_err(|e| PluginError::MonitorCreate(e.to_string()))?,
    );

    tracing::info!("Created plugins monitor");

    // Create chat monitor
    let chat_monitor = Arc::new(
        client
            .create_chat_monitor()
            .await
            .map_err(|e| PluginError::MonitorCreate(e.to_string()))?,
    );

    tracing::info!("Created chat monitor");

    // Subscribe to all chats
    chat_monitor
        .subscribe_to_all_chats()
        .await
        .map_err(|e| PluginError::Subscription(e.to_string()))?;

    tracing::info!("Subscribed to all chats");

    // Create AI client
    let default_model = &config.ai_completions.models["default"];
    let model_name = default_model.alias.as_ref().unwrap_or(&"default".to_string());
    let model_config = &config.ai_completions.models[model_name];
    
    let api_key = config::resolve_api_key(&config, &model_config.api_key.cred)
        .map_err(|e| PluginError::Config(e.to_string()))?;
    
    let ai_client = Arc::new(AiClient::new(
        model_config.base_url.as_ref().unwrap_or(&String::new()),
        api_key,
    ));

    tracing::info!("Created AI client");

    // Main loop: check for trigger conditions and handle AI requests
    loop {
        // Get all chat IDs
        let chat_ids = chat_monitor.get_chat_ids().await;

        for chat_id in chat_ids {
            // Get chat state
            if let Some(chat_state) = chat_monitor.get_chat_state(chat_id).await {
                // Check trigger condition
                let trigger_reason = trigger_detection::should_trigger(&chat_state);
                
                if trigger_reason != trigger_detection::TriggerReason::None {
                    tracing::info!(
                        "Triggering AI completion for chat {} with reason {:?}",
                        chat_id,
                        trigger_reason
                    );

                    // Handle AI request
                    if let Err(e) = ai_request::handle_ai_request(
                        Arc::clone(&client),
                        Arc::clone(&plugins_monitor),
                        Arc::clone(&ai_client),
                        &config,
                        plugin_id,
                        chat_id,
                        &chat_state.messages,
                        trigger_reason,
                    )
                    .await
                    {
                        tracing::error!("AI request failed for chat {}: {}", chat_id, e);
                    }
                }
            }
        }

        // Wait before next check
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("failed to connect to server: {0}")]
    Connection(String),
    #[error("failed to register plugin: {0}")]
    Registration(String),
    #[error("failed to get pending acks: {0}")]
    PendingAcks(String),
    #[error("failed to create monitor: {0}")]
    MonitorCreate(String),
    #[error("failed to subscribe: {0}")]
    Subscription(String),
    #[error("configuration error: {0}")]
    Config(String),
}
```

## Tests

### Unit Tests for `tool_resolution.rs`

```rust
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
    fn test_no_tool_calls() {
        let messages = vec![
            create_message(1, "user", "Hello"),
            create_message(2, "assistant", "Hi there"),
        ];
        assert!(!has_unresolved_tool_calls(&messages));
        assert!(!all_tool_calls_resolved(&messages));
    }

    // Add more tests once tool call detection is implemented
}
```

### Unit Tests for `ai_request.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_filter_messages_for_ai() {
        let messages = vec![
            Message {
                id: 1,
                chat_id: 1,
                role: "user".to_string(),
                content: "Hello".to_string(),
                created_at: Utc::now(),
                reasoning_content: None,
                tags: vec![],
            },
            Message {
                id: 2,
                chat_id: 1,
                role: "ai_completions:error".to_string(),
                content: "Error".to_string(),
                created_at: Utc::now(),
                reasoning_content: None,
                tags: vec![],
            },
            Message {
                id: 3,
                chat_id: 1,
                role: "assistant".to_string(),
                content: "Response".to_string(),
                created_at: Utc::now(),
                reasoning_content: None,
                tags: vec![],
            },
        ];

        let filtered = filter_messages_for_ai(&messages);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].role, "user");
        assert_eq!(filtered[1].role, "assistant");
    }
}
```

## Implementation Notes

1. **Tool Call Detection**: The `tool_resolution.rs` module has placeholder implementations for `has_tool_calls` and `extract_tool_call_ids`. These need to be implemented based on how tool calls are represented in messages. This may require:
   - Adding a `tool_calls` field to the `Message` struct in `rhd_chat_api`
   - Parsing tool calls from message content
   - Using a different detection mechanism

2. **Trigger Loop**: The main loop checks for trigger conditions every second. This is simple but may need optimization for production use.

3. **Error Handling**: All errors are properly propagated and logged. The plugin continues running even if individual requests fail.

4. **Memory Management**: The ChatMonitor (from rhd_chat_client) stores state for all chats. For large numbers of chats, this may need optimization.

5. **Concurrency**: All shared state is protected by `RwLock` or `Arc` to ensure thread safety.

6. **AI Client Configuration**: The AI client is configured from the plugin config, resolving credentials from the credentials file.

## Dependencies

- **Depends on**: Phase 1, Phase 2, Phase 3
- **Must be completed before**: Phase 5 (integration testing)

## Success Criteria

- [ ] `tool_resolution.rs` detects tool call resolution correctly
- [ ] `trigger_detection.rs` detects trigger conditions using ChatMonitor from rhd_chat_client
- [ ] `ai_request.rs` handles AI requests with preRequest event coordination
- [ ] `plugin.rs` integrates all components and runs main loop
- [ ] Error handling adds error tags and messages correctly
- [ ] Queued messages are processed correctly
- [ ] All unit tests pass
- [ ] Code compiles without warnings
