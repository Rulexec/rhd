# AI Completions Running Tag Implementation Plan

## Goal

Add a new `ai_completions:running` tag to chats in the `rhd_plugin_ai_completions` plugin to provide visibility into when an AI request is actively in progress. This tag allows other plugins and UI components to know when a chat is being processed by the AI completions plugin.

## Background

Currently, the plugin only adds the `ai_completions:error` tag when requests fail. There is no visibility into when a request is actively being processed. The new `ai_completions:running` tag will:
- Be added right before acknowledging the plugin's own `preRequest` event
- Be removed when the request completes without tool calls
- Be removed when transitioning to error state
- Stay present during tool loop iterations

## Architecture

### Tag Lifecycle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          AI Request Flow                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. Send preRequest event                                                    │
│         │                                                                    │
│         ▼                                                                    │
│  2. Wait for other plugins' acks                                             │
│         │                                                                    │
│         ▼                                                                    │
│  3. Process queued messages (if applicable)                                  │
│         │                                                                    │
│         ▼                                                                    │
│  4. ADD `ai_completions:running` TAG  ◄── NEW                                │
│         │                                                                    │
│         ▼                                                                    │
│  5. Acknowledge own event                                                    │
│         │                                                                    │
│         ▼                                                                    │
│  6. Build AI request                                                         │
│         │                                                                    │
│         ├──► FAIL: Remove `running`, add `error`                             │
│         │                                                                    │
│         ▼                                                                    │
│  7. Make AI request                                                          │
│         │                                                                    │
│         ├──► FAIL: Remove `running`, add `error`                             │
│         │                                                                    │
│         ▼                                                                    │
│  8. Stream response                                                          │
│         │                                                                    │
│         ├──► ERROR: Remove `running`, add `error`                            │
│         │                                                                    │
│         ▼                                                                    │
│  9. Stream finished                                                          │
│         │                                                                    │
│         ├──► Has tool calls: Keep `running` (tool loop continues)            │
│         │                                                                    │
│         └──► No tool calls: Remove `running`                                 │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Startup Reconciliation

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    Startup Reconciliation                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  For each chat with `ai_completions:running` tag:                           │
│         │                                                                    │
│         ├──► Has unfinished message:                                         │
│         │    - Add `ai_completions:error`                                    │
│         │    - Remove `ai_completions:running`                               │
│         │                                                                    │
│         └──► Eligible for request (passes should_trigger):                   │
│              - Remove `ai_completions:running` only                          │
│              - Let normal flow continue                                      │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Implementation Steps

### Step 1: Add `ai_completions:running` tag before own ack

**File:** `plugins/rhd_plugin_ai_completions/src/ai_request.rs`

**Location:** Before line 143 (own event acknowledgment)

**Change:**
```rust
// Add running tag before acknowledging own event
client
    .update_chat(UpdateChatParams {
        chat_id,
        title: None,
        add_tags: vec!["ai_completions:running".to_string()],
        remove_tags: vec![],
    })
    .await
    .map_err(|e| {
        tracing::error!(chat_id = chat_id, error = %e, "failed to add running tag");
        AiRequestError::TagAdd(e.to_string())
    })?;

// Acknowledge own event
tracing::debug!(
    chat_id = chat_id,
    event_id = %event_id,
    "acknowledging own event"
);
```

### Step 2: Remove `running` tag on message conversion failure

**File:** `plugins/rhd_plugin_ai_completions/src/ai_request.rs`

**Location:** Line 167-182 (message conversion error handling)

**Change:**
```rust
client
    .update_chat(UpdateChatParams {
        chat_id,
        title: None,
        add_tags: vec!["ai_completions:error".to_string()],
        remove_tags: vec!["ai_completions:running".to_string()],  // ADD THIS
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
```

### Step 3: Remove `running` tag on streaming error

**File:** `plugins/rhd_plugin_ai_completions/src/ai_request.rs`

**Location:** Line 327-340 (streaming error handling)

**Change:** Add `remove_tags` to the `update_message` call. Note: The error tag is added to the message, not the chat. We need to also update the chat to remove the running tag.

```rust
// Remove running tag from chat on streaming error
let _ = client
    .update_chat(UpdateChatParams {
        chat_id,
        title: None,
        add_tags: vec![],
        remove_tags: vec!["ai_completions:running".to_string()],
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
```

### Step 4: Remove `running` tag on AI request failure

**File:** `plugins/rhd_plugin_ai_completions/src/ai_request.rs`

**Location:** Line 426-437 (AI request failure before streaming)

**Change:**
```rust
// Add error tag to chat and remove running tag
client
    .update_chat(UpdateChatParams {
        chat_id,
        title: None,
        add_tags: vec!["ai_completions:error".to_string()],
        remove_tags: vec!["ai_completions:running".to_string()],  // ADD THIS
    })
    .await
    .map_err(|e| {
        tracing::error!(chat_id = chat_id, error = %e, "failed to add error tag");
        AiRequestError::TagAdd(e.to_string())
    })?;
```

### Step 5: Remove `running` tag on successful completion without tool calls

**File:** `plugins/rhd_plugin_ai_completions/src/ai_request.rs`

**Location:** After line 369 (stream finish), before or after message update

**Change:**
```rust
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

// Remove running tag if no tool calls (tool loop will continue otherwise)
if final_tool_calls.is_empty() {
    client
        .update_chat(UpdateChatParams {
            chat_id,
            title: None,
            add_tags: vec![],
            remove_tags: vec!["ai_completions:running".to_string()],
        })
        .await
        .map_err(|e| {
            tracing::error!(chat_id = chat_id, error = %e, "failed to remove running tag");
            AiRequestError::TagAdd(e.to_string())
        })?;
}
```

### Step 6: Update startup reconciliation

**File:** `plugins/rhd_plugin_ai_completions/src/plugin.rs`

**Location:** `tag_crashed_chats()` function (line 209-247)

**Change:** Add handling for chats with `ai_completions:running` tag

```rust
/// Once at startup, reconcile chats that may have been left in an inconsistent state.
///
/// For chats with `ai_completions:running` tag:
/// - If chat has unfinished message → add error tag, remove running tag
/// - If chat is eligible for request → remove running tag only
///
/// For chats without running tag but with unfinished message:
/// - Add error tag (existing behavior)
async fn tag_crashed_chats(
    client: &ChatClient,
    chat_monitor: &ChatMonitor,
) -> Result<(), PluginError> {
    for chat_id in chat_monitor.get_chat_ids().await {
        let Some(state) = chat_monitor.get_chat_state(chat_id).await else {
            continue;
        };

        let has_running_tag = state.tags.iter().any(|tag| tag == "ai_completions:running");
        let has_error_tag = state.tags.iter().any(|tag| tag == "ai_completions:error");

        // Already parked — nothing to do
        if has_error_tag {
            continue;
        }

        // Check for unfinished message
        let has_unfinished = find_unfinished_message(&state.messages).is_some();

        if has_running_tag {
            if has_unfinished {
                // Chat was left mid-stream → park it
                tracing::warn!(
                    chat_id = chat_id,
                    "chat has running tag and unfinished message; parking with error tag"
                );
                client
                    .update_chat(UpdateChatParams {
                        chat_id,
                        title: None,
                        add_tags: vec!["ai_completions:error".to_string()],
                        remove_tags: vec!["ai_completions:running".to_string()],
                    })
                    .await
                    .map_err(|e| PluginError::StartupReconciliation(e.to_string()))?;
            } else {
                // Chat is in good state, just remove running tag
                tracing::info!(
                    chat_id = chat_id,
                    "chat has running tag but is eligible; removing running tag"
                );
                client
                    .update_chat(UpdateChatParams {
                        chat_id,
                        title: None,
                        add_tags: vec![],
                        remove_tags: vec!["ai_completions:running".to_string()],
                    })
                    .await
                    .map_err(|e| PluginError::StartupReconciliation(e.to_string()))?;
            }
        } else if has_unfinished {
            // Existing behavior: park chats with unfinished messages
            let offender = find_unfinished_message(&state.messages).unwrap();
            tracing::warn!(
                chat_id = chat_id,
                message_id = offender.id,
                is_streaming = offender.is_streaming,
                is_finished = offender.is_finished,
                "unfinished message found at startup; parking chat with error tag"
            );

            client
                .update_chat(UpdateChatParams {
                    chat_id,
                    title: None,
                    add_tags: vec!["ai_completions:error".to_string()],
                    remove_tags: vec![],
                })
                .await
                .map_err(|e| PluginError::StartupReconciliation(e.to_string()))?;
        }
    }

    Ok(())
}
```

### Step 7: Update README documentation

**File:** `plugins/rhd_plugin_ai_completions/README.md`

**Location:** After the `ai_completions:error` section (line 76-85)

**Add new section:**
```markdown
### `ai_completions:running`

Added to a chat when the plugin is actively processing an AI request.

**When added**: Right before acknowledging the plugin's own `ai_completions:preRequest` event.

**When removed**:
1. When the AI request completes successfully and the response has no tool calls
2. When transitioning to `ai_completions:error` state (request failure, message conversion failure, streaming error)
3. At plugin startup, if the chat is eligible for a new request

**When kept**:
- When the AI response contains tool calls (tool loop continues)
- If the plugin crashes or fails to acknowledge its own event (chat considered broken until restart)

**Effect**: Provides visibility into when a chat is being processed. Other plugins can use this tag to:
- Display loading indicators in UI
- Avoid conflicting operations on the chat
- Monitor plugin activity

**Startup reconciliation**: At plugin startup, chats with this tag are checked:
- If the chat has an unfinished message (crash recovery) → add `ai_completions:error`, remove `ai_completions:running`
- If the chat is eligible for a request → remove `ai_completions:running` only, let normal flow continue
```

## File Changes Summary

| File | Changes |
|------|---------|
| `plugins/rhd_plugin_ai_completions/src/ai_request.rs` | Add `running` tag before ack; remove `running` tag on all error paths; remove `running` tag on success without tool calls |
| `plugins/rhd_plugin_ai_completions/src/plugin.rs` | Update `tag_crashed_chats()` to handle `running` tag at startup |
| `plugins/rhd_plugin_ai_completions/README.md` | Document new `ai_completions:running` tag behavior |

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Tag operations fail silently | Log errors for all tag operations; existing error handling will propagate failures |
| Plugin crashes during request | Startup reconciliation handles both unfinished messages and eligible chats |
| Ack fails after adding `running` tag | Chat considered broken; startup reconciliation will handle it |
| Race condition with other plugins | Tag operations are atomic via `UpdateChatParams` |
| Tool loop never terminates | Existing behavior unchanged; `running` tag stays as expected |

## Success Criteria

1. **Functional**:
   - [ ] `ai_completions:running` tag is added before own ack
   - [ ] `ai_completions:running` tag is removed on message conversion failure
   - [ ] `ai_completions:running` tag is removed on streaming error
   - [ ] `ai_completions:running` tag is removed on AI request failure
   - [ ] `ai_completions:running` tag is removed on success without tool calls
   - [ ] `ai_completions:running` tag stays on success with tool calls
   - [ ] Startup reconciliation handles chats with `running` tag correctly

2. **Testing**:
   - [ ] Unit tests for tag operations in `ai_request.rs`
   - [ ] Integration tests for startup reconciliation
   - [ ] Manual testing of full flow

3. **Documentation**:
   - [ ] README updated with new tag behavior
   - [ ] Code comments explain tag lifecycle

## Testing Approach

### Unit Tests

Add tests to verify:
1. `running` tag added before ack call
2. `running` tag removed when `error` tag added (all 3 paths)
3. `running` tag removed on success without tool calls
4. `running` tag stays on success with tool calls

### Integration Tests

Add tests to verify startup reconciliation:
1. Chat with `running` tag + unfinished message → error tag added, running removed
2. Chat with `running` tag + eligible state → running tag removed only

### Manual Testing

1. Trigger AI request, verify tag appears during request
2. Verify tag disappears after response without tool calls
3. Verify tag stays after response with tool calls
4. Kill plugin during request, restart, verify error tag added for unfinished message
5. Kill plugin during request, restart with eligible chat, verify running tag removed

## Dependencies

- No new dependencies required
- Uses existing `UpdateChatParams` API

## Rollout Plan

1. Implement changes in `ai_request.rs`
2. Implement changes in `plugin.rs`
3. Update README documentation
4. Add unit tests
5. Add integration tests
6. Manual testing
7. Code review
8. Merge to main
