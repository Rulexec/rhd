# AI Request chatId Field Move

## Summary

Move `chatId` from the JSON payload in the `additional` field to the dedicated `chat_id` field of `SendCustomEventParams` in the `ai_completions:preRequest` event.

## Current State

In [`plugins/rhd_plugin_ai_completions/src/ai_request.rs`](plugins/rhd_plugin_ai_completions/src/ai_request.rs:49):

```rust
.send_custom_event(SendCustomEventParams {
    event_name: "ai_completions:preRequest".to_string(),
    additional: Some(
        serde_json::to_string(&serde_json::json!({
            "chatId": chat_id,  // <-- Line 53: Remove this
            "triggerReason": match trigger_reason {
                TriggerReason::QueuedMessages => "queuedMessages",
                TriggerReason::ToolLoopContinuation => "toolLoopContinuation",
                TriggerReason::None => "none",
            }
        }))
        .map_err(|e| AiRequestError::EventSend(e.to_string()))?,
    ),
    chat_id: None,  // <-- Line 62: Change to Some(chat_id.to_string())
    message_id: None,
    tool_call_id: None,
})
```

## Target State

```rust
.send_custom_event(SendCustomEventParams {
    event_name: "ai_completions:preRequest".to_string(),
    additional: Some(
        serde_json::to_string(&serde_json::json!({
            "triggerReason": match trigger_reason {
                TriggerReason::QueuedMessages => "queuedMessages",
                TriggerReason::ToolLoopContinuation => "toolLoopContinuation",
                TriggerReason::None => "none",
            }
        }))
        .map_err(|e| AiRequestError::EventSend(e.to_string()))?,
    ),
    chat_id: Some(chat_id.to_string()),  // <-- Moved here
    message_id: None,
    tool_call_id: None,
})
```

## Type Conversion

- `SendCustomEventParams.chat_id` is `Option<String>`
- The `chat_id` variable in the function is `i64`
- Conversion: `Some(chat_id.to_string())`

## Consumer Analysis

Searched for code that parses the `additional` field of custom events:
- No consumers found that read `chatId` from the JSON payload
- The `chatId` field in the JSON payload is not used anywhere in the codebase

## Implementation Steps

1. Remove `"chatId": chat_id,` from the JSON object at line 53
2. Change `chat_id: None,` to `chat_id: Some(chat_id.to_string()),` at line 62
3. No consumer updates needed (no consumers found)

## Files Modified

- `plugins/rhd_plugin_ai_completions/src/ai_request.rs`
