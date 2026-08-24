# Phase 4: rhd_plugin_ai_completions Streaming Integration

## Overview

Update the AI completions plugin to use streaming AI requests and the new stream API. The plugin will create a message with `is_streaming: true` before starting the AI request, push deltas to the stream during generation, and finish the stream when complete.

## Scope

- Modify `handle_ai_request` to use streaming AI requests
- Create message with `is_streaming: true, is_finished: false` before AI request
- Push reasoning/content/tool_call deltas to stream during generation
- Call `streamFinish` when streaming completes
- Update message with final content and `is_streaming: false, is_finished: true`
- Handle errors gracefully (always finish the stream)

## Dependencies

- Phase 2 (stream manager in server).
- Phase 3 (stream API methods).

---

## Files to Modify

### 1. `plugins/rhd_plugin_ai_completions/src/ai_request.rs`

**Rewrite `handle_ai_request` function:**

The current implementation makes a non-streaming request and adds the complete response as a single message. The new implementation:

1. Creates a message with `is_streaming: true, is_finished: false` before the AI request.
2. Makes a streaming AI request (`stream: true`).
3. For each chunk from the AI provider, pushes deltas to the stream.
4. When streaming completes, calls `streamFinish` and updates the message with final content.

**New implementation:**

```rust
use rhd_chat_api::{
    AddMessageParams, StreamPushParams, StreamFinishParams, StreamToolCallDelta,
    UpdateMessageParams,
};

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
    // Send preRequest event (unchanged)
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

    // Wait for acknowledgments (unchanged)
    let except_plugins: Vec<&str> = vec![plugin_id];
    plugins_monitor
        .wait_for_acks_except(&event_id, &except_plugins, Duration::from_secs(30))
        .await
        .map_err(|e| AiRequestError::WaitTimeout(e.to_string()))?;

    // Process queued messages if needed (unchanged)
    let current_messages = if trigger_reason == TriggerReason::QueuedMessages {
        process_queued_messages(&client, chat_id).await?;
        let chat_result = client
            .get_chat(GetChatParams {
                chat_id,
                if_version_higher_than: known_version,
            })
            .await
            .map_err(|e| AiRequestError::MessageAdd(e.to_string()))?;
        chat_result.messages
    } else {
        // Verify state freshness (unchanged)
        if let Some(version) = known_version {
            let chat_result = client
                .get_chat(GetChatParams {
                    chat_id,
                    if_version_higher_than: Some(version),
                })
                .await
                .map_err(|e| AiRequestError::MessageAdd(e.to_string()))?;
            
            if chat_result.chat.version > version {
                chat_result.messages
            } else {
                messages.to_vec()
            }
        } else {
            messages.to_vec()
        }
    };

    // Acknowledge own event (unchanged)
    client
        .ack_custom_event(AckCustomEventParams {
            event_id: event_id.clone(),
        })
        .await
        .map_err(|e| AiRequestError::EventAck(e.to_string()))?;

    // Build AI request (unchanged)
    let filtered_messages = filter_messages_for_ai(&current_messages);
    let ai_messages = convert_to_ai_messages(&filtered_messages);

    let default_model = &config.ai_completions.models["default"];
    let default_name = "default".to_string();
    let model_name = default_model.alias.as_ref().unwrap_or(&default_name);
    let model_config = &config.ai_completions.models[model_name];

    // Create message with streaming flags BEFORE AI request
    let add_result = client
        .add_message(AddMessageParams {
            chat_id,
            role: "assistant".to_string(),
            content: String::new(),
            reasoning_content: None,
            tags: vec![],
            is_finished: false,
            is_streaming: true,
        })
        .await
        .map_err(|e| AiRequestError::MessageAdd(e.to_string()))?;

    let message_id = add_result.message_id;
    tracing::info!(chat_id = chat_id, message_id = message_id, "created streaming message");

    // Make streaming AI request
    let request = ChatCompletionRequest {
        model: model_config.model.clone().unwrap_or_else(|| "default".to_string()),
        messages: ai_messages,
        tools: None,
        stream: true, // Enable streaming
    };

    let request_body = serde_json::to_string(&request)
        .map_err(|e| AiRequestError::EventSend(e.to_string()))?;
    tracing::info!(chat_id = chat_id, request_body = %request_body, "sending streaming AI request");

    // Accumulate final content
    let mut final_reasoning = String::new();
    let mut final_content = String::new();
    let mut final_tool_calls: Vec<StreamToolCallDelta> = Vec::new();

    match ai_client.chat_completion_stream(request).await {
        Ok(mut stream) => {
            // Process each chunk from the stream
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        // Extract deltas from chunk
                        if let Some(choice) = chunk.choices.first() {
                            let delta = &choice.delta;

                            // Reasoning content delta
                            if let Some(reasoning) = &delta.reasoning_content {
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
                            if let Some(content) = &delta.content {
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
                            if let Some(tool_calls) = &delta.tool_calls {
                                let mut tool_deltas = Vec::new();
                                for tc in tool_calls {
                                    let id = tc.id.clone().unwrap_or_default();
                                    let name = tc.function.as_ref().map(|f| f.name.clone()).unwrap_or_default();
                                    let arguments = tc.function.as_ref().map(|f| f.arguments.clone()).unwrap_or_default();

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
                                        name: tc.function.as_ref().map(|f| f.name.clone()).unwrap_or_default(),
                                        arguments: tc.function.as_ref().map(|f| f.arguments.clone()).unwrap_or_default(),
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
                        function: rhd_chat_api::FunctionCall {
                            name: tc.name.clone(),
                            arguments: tc.arguments.clone(),
                        },
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
```

**Add new error variants to `AiRequestError`:**

```rust
#[derive(Debug, thiserror::Error)]
pub enum AiRequestError {
    // ... existing variants ...
    #[error("failed to push to stream: {0}")]
    StreamPush(String),
    #[error("failed to finish stream: {0}")]
    StreamFinish(String),
    #[error("failed to update message: {0}")]
    MessageUpdate(String),
}
```

---

### 2. `plugins/rhd_plugin_ai_completions/Cargo.toml`

**Add dependency on `futures` for stream iteration:**

```toml
[dependencies]
# ... existing dependencies ...
futures = "0.3"
```

---

### 3. `packages/rhd_ai_client/src/client.rs`

**Add `chat_completion_stream` method:**

The `AiClient` needs a method that returns a stream of chunks instead of a complete response.

```rust
use futures::Stream;

impl AiClient {
    /// Make a streaming chat completion request.
    /// Returns a stream of chunks.
    pub async fn chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<impl Stream<Item = Result<StreamChunk, AiClientError>>, AiClientError> {
        // Implementation depends on the HTTP client being used.
        // This is a placeholder — the actual implementation will need to:
        // 1. Send the request with stream: true
        // 2. Parse the SSE (Server-Sent Events) response
        // 3. Yield each chunk as a StreamChunk
        // 4. Handle errors appropriately
        
        todo!("Implement streaming chat completion")
    }
}
```

**Note**: The actual implementation of `chat_completion_stream` depends on the HTTP client and SSE parsing logic. This may require additional work in `rhd_ai_client` to support streaming responses. If streaming is not yet supported in `rhd_ai_client`, this phase may need to be split into a separate sub-task.

---

## Tests

### Integration Tests

1. **`test_streaming_ai_request`**:
   - Mock AI provider returns a streaming response with reasoning, content, and tool calls.
   - Verify message is created with `is_streaming: true`.
   - Verify stream receives all deltas.
   - Verify message is updated with final content and `is_streaming: false, is_finished: true`.

2. **`test_streaming_error_handling`**:
   - Mock AI provider returns an error during streaming.
   - Verify stream is finished.
   - Verify message is updated with error content and error tag.

3. **`test_streaming_tool_calls`**:
   - Mock AI provider returns tool call deltas.
   - Verify tool calls are accumulated correctly.
   - Verify final message contains the complete tool calls.

---

## Implementation Notes

1. **Message creation before AI request**: The message is created with `is_streaming: true, is_finished: false` BEFORE the AI request starts. This allows the frontend to immediately show a streaming indicator.

2. **Stream push errors are fatal**: If `stream_push` fails, the entire AI request fails. This ensures the frontend doesn't wait indefinitely for a stream that will never complete.

3. **Tool call accumulation**: Tool call deltas are accumulated by ID. If multiple deltas have the same ID, their arguments are concatenated. This handles the case where the AI provider sends tool call arguments in multiple chunks.

4. **Final content vs accumulated deltas**: The `streamFinish` call passes the final accumulated content. The server can use this to override the accumulated deltas if needed (e.g., if the AI provider returns final tool calls that differ from streaming deltas).

5. **Error handling**: Errors during streaming are handled gracefully:
   - The stream is always finished (even on error).
   - The message is updated with error content and the `ai_completions:error` tag.
   - The error is logged and returned to the caller.

6. **Streaming support in `rhd_ai_client`**: This phase assumes `rhd_ai_client` already supports streaming responses. If not, a separate task is needed to add `chat_completion_stream` method to `AiClient`.
