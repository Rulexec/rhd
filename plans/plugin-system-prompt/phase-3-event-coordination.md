# Phase 3: Event Coordination

## Overview

This phase implements the `ai_completions:preRequest` event handling with proper acknowledgment timing. This ensures the plugin coordinates correctly with the AI completions plugin.

**Scope:**
- Subscribe to custom events
- Filter for `ai_completions:preRequest` events
- Extract chat ID from event data
- Implement acknowledgment logic with proper timing
- Handle race conditions with unfinished messages

**Out of Scope:**
- Integration tests (covered in Phase 4)

## Files to Modify

### 1. `plugins/rhd_plugin_system_prompt/src/plugin.rs`

**Modifications:**

Add custom event subscription and handling logic to the `run_plugin` function.

**Add after "Subscribed to all chats" log message:**

```rust
// Subscribe to custom events for coordination with AI completions plugin
let client_for_events = Arc::clone(&client);
let chat_monitor_for_events = Arc::clone(&chat_monitor);
let cached_prompts_for_events = cached_prompts.clone();
let plugin_id_for_events = plugin_id.to_string();

client.on_custom_event(move |event| {
    let client = Arc::clone(&client_for_events);
    let chat_monitor = Arc::clone(&chat_monitor_for_events);
    let cached_prompts = cached_prompts_for_events.clone();
    let plugin_id = plugin_id_for_events.clone();
    
    async move {
        // Only handle ai_completions:preRequest events
        if event.event_name != "ai_completions:preRequest" {
            return;
        }

        tracing::info!(
            event_id = %event.event_id,
            "received ai_completions:preRequest event"
        );

        // Extract chat ID from event
        let chat_id = match extract_chat_id_from_event(&event) {
            Some(id) => id,
            None => {
                tracing::warn!(
                    event_id = %event.event_id,
                    "event missing chatId, acknowledging without action"
                );
                // Still acknowledge to avoid blocking
                let _ = client.ack_custom_event(rhd_chat_api::AckCustomEventParams {
                    event_id: event.event_id,
                    is_rejected: None,
                }).await;
                return;
            }
        };

        // Get chat state
        let chat_state = match chat_monitor.get_chat_state(chat_id).await {
            Some(state) => state,
            None => {
                tracing::warn!(
                    chat_id = chat_id,
                    "chat not found in monitor, acknowledging without action"
                );
                let _ = client.ack_custom_event(rhd_chat_api::AckCustomEventParams {
                    event_id: event.event_id,
                    is_rejected: None,
                }).await;
                return;
            }
        };

        // Check if chat has unfinished assistant message
        if system_prompt::has_unfinished_assistant_message(&chat_state) {
            tracing::debug!(
                chat_id = chat_id,
                "chat has unfinished assistant message, acknowledging without injection"
            );
            let _ = client.ack_custom_event(rhd_chat_api::AckCustomEventParams {
                event_id: event.event_id,
                is_rejected: None,
            }).await;
            return;
        }

        // Check if we need to inject system prompts
        let required_names = system_prompt::get_required_prompt_names(&chat_state);
        let mut needs_injection = false;

        for prompt_name in &required_names {
            if !system_prompt::has_system_prompt_message(&chat_state, prompt_name) {
                needs_injection = true;
                break;
            }
        }

        if needs_injection {
            // Inject system prompts BEFORE acknowledging
            tracing::info!(
                chat_id = chat_id,
                "injecting system prompts before acknowledging event"
            );

            match system_prompt::process_chat(&client, &chat_state, &cached_prompts).await {
                Ok(count) => {
                    tracing::info!(
                        chat_id = chat_id,
                        injected_count = count,
                        "injected system prompts before ack"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %e,
                        "failed to inject system prompts, acknowledging anyway"
                    );
                }
            }
        }

        // Acknowledge the event
        match client.ack_custom_event(rhd_chat_api::AckCustomEventParams {
            event_id: event.event_id,
            is_rejected: None,
        }).await {
            Ok(_) => {
                tracing::info!(
                    event_id = %event.event_id,
                    chat_id = chat_id,
                    "acknowledged ai_completions:preRequest event"
                );
            }
            Err(e) => {
                tracing::error!(
                    event_id = %event.event_id,
                    chat_id = chat_id,
                    error = %e,
                    "failed to acknowledge event"
                );
            }
        }
    }
});

tracing::info!("Subscribed to custom events for coordination");
```

**Add helper function at the end of the file (before the `PluginError` enum):**

```rust
/// Extract chat ID from a custom event's additional data.
///
/// The AI completions plugin includes `chatId` in the event's additional JSON field.
fn extract_chat_id_from_event(event: &rhd_chat_api::CustomEventData) -> Option<i64> {
    let additional = event.additional.as_ref()?;
    let json: serde_json::Value = serde_json::from_str(additional).ok()?;
    let chat_id_str = json.get("chatId")?.as_str()?;
    chat_id_str.parse::<i64>().ok()
}
```

**Key Implementation Details:**

1. **Event Subscription**: Use `client.on_custom_event()` to subscribe to all custom events, then filter for `ai_completions:preRequest` events by checking `event.event_name`.

2. **Chat ID Extraction**: The AI completions plugin includes `chatId` in the event's `additional` JSON field. We parse this to get the chat ID.

3. **Acknowledgment Logic**:
   - **Always acknowledge** the event (never reject or timeout)
   - **If chat has unfinished assistant message**: Ack immediately, don't add system prompt (conditions not met)
   - **If chat needs system prompt and not yet added**: Add system prompt FIRST, then ack
   - **If system prompt already exists OR not needed**: Ack immediately

4. **Synchronous Injection**: When system prompt needs to be added, wait for the `process_chat` call to complete before acknowledging. This ensures the system prompt is in place before the AI request proceeds.

5. **Error Handling**: If system prompt injection fails, log the error but still acknowledge the event. This prevents blocking the AI request indefinitely.

## Implementation Notes

1. **Event Flow**:
   - AI completions plugin sends `ai_completions:preRequest` event
   - System prompt plugin receives the event
   - System prompt plugin checks if injection is needed
   - If needed, inject system prompts
   - Acknowledge the event
   - AI completions plugin waits for all acks before proceeding

2. **Race Condition Prevention**: By injecting system prompts BEFORE acknowledging, we ensure that the AI request won't proceed until the system prompt is in place.

3. **Unfinished Message Handling**: If the chat has an unfinished assistant message, we acknowledge immediately without injecting. This is because:
   - The AI completions plugin shouldn't be making requests while a message is unfinished
   - We can't safely add a system prompt while streaming is in progress
   - Acknowledging allows the system to continue (even if it's in an unexpected state)

4. **Error Resilience**: Even if system prompt injection fails, we still acknowledge the event. This prevents the AI completions plugin from being blocked indefinitely.

5. **Logging**: Extensive logging is included to help debug coordination issues between plugins.

## Dependencies

- **Phase 2** - Requires core plugin logic and system prompt injection methods
- This phase must be completed before Phase 4

## Success Criteria

- [ ] Plugin receives `ai_completions:preRequest` events
- [ ] System prompts are injected before acknowledgment when needed
- [ ] Events are acknowledged in all cases (no timeouts)
- [ ] Unfinished messages are handled correctly (ack without injection)
- [ ] Missing chat ID is handled gracefully (ack without action)
- [ ] Injection errors don't block the AI request
- [ ] Coordination with AI completions plugin works correctly
