# Plugin Event-Driven Refactor

## Problem Statement

Both `rhd_plugin_ai_completions` and `rhd_plugin_system_prompt` use polling loops that check ALL monitored chats every second:

```rust
loop {
    for chat_id in chat_monitor.get_chat_ids().await {
        // Check trigger conditions for EVERY chat
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
}
```

This pattern is inefficient because:
- **O(n) work per second** where n = number of chats (will grow over time)
- **Most iterations find no changes** — wasted CPU cycles
- **Doesn't scale** — as chat count grows, the plugin becomes a bottleneck
- **Latency** — up to 1 second delay between event and trigger

The `ChatMonitor` already receives events and updates state, but plugins don't react to those events — they poll instead.

## Solution

Add an event notification mechanism to `ChatMonitor` that allows plugins to register callbacks for chat state changes. Plugins will only check trigger conditions for chats that actually changed.

## Implementation Plan

### Phase 1: Add Event Notification to ChatMonitor

**File:** `packages/rhd_chat_client/src/chat_monitor.rs`

Add a callback registration mechanism:

```rust
pub struct ChatMonitor {
    client: Arc<ChatClient>,
    chat_states: Arc<RwLock<HashMap<i64, ChatState>>>,
    _chats_list_token: CancellationToken,
    chat_tokens: Arc<RwLock<Vec<CancellationToken>>>,
    // NEW: callbacks to notify when chat state changes
    state_change_callbacks: Arc<RwLock<Vec<Box<dyn Fn(i64, ChatState) + Send + Sync>>>>,
}

impl ChatMonitor {
    /// Register a callback to be notified when a chat's state changes.
    /// The callback receives the chat_id and the new ChatState.
    pub async fn on_chat_state_change<F>(&self, callback: F)
    where
        F: Fn(i64, ChatState) + Send + Sync + 'static,
    {
        self.state_change_callbacks
            .write()
            .await
            .push(Box::new(callback));
    }

    // Internal method to notify all callbacks
    async fn notify_state_change(&self, chat_id: i64, state: ChatState) {
        let callbacks = self.state_change_callbacks.read().await;
        for callback in callbacks.iter() {
            callback(chat_id, state.clone());
        }
    }
}
```

**Modify event handlers** to call `notify_state_change` after updating state:
- In `on_chat_event` handlers (lines ~167-191 and ~338-361)
- In `on_chats_list_event` handlers (lines ~53-218)

After each state update, call:
```rust
if let Some(state) = states.read().await.get(&chat_id).cloned() {
    self.notify_state_change(chat_id, state).await;
}
```

### Phase 2: Refactor rhd_plugin_ai_completions

**File:** `plugins/rhd_plugin_ai_completions/src/plugin.rs`

**Remove the polling loop** (lines 136-211) and replace with event-driven logic:

```rust
// Register callback for chat state changes
let processing_chats_clone = Arc::clone(&processing_chats);
let client_for_callback = Arc::clone(&client);
let plugins_monitor_for_callback = Arc::clone(&plugins_monitor);
let ai_client_for_callback = Arc::clone(&ai_client);
let config_for_callback = config.clone();
let plugin_id_for_callback = plugin_id.to_string();

chat_monitor.on_chat_state_change(move |chat_id, chat_state| {
    let processing_chats = Arc::clone(&processing_chats_clone);
    let client = Arc::clone(&client_for_callback);
    let plugins_monitor = Arc::clone(&plugins_monitor_for_callback);
    let ai_client = Arc::clone(&ai_client_for_callback);
    let config = config_for_callback.clone();
    let plugin_id = plugin_id_for_callback.clone();
    let chat_state_clone = chat_state.clone();

    // Spawn async task to handle the trigger check
    tokio::spawn(async move {
        // Check trigger condition
        let trigger_reason = trigger_detection::should_trigger(&chat_state_clone);

        if trigger_reason == trigger_detection::TriggerReason::None {
            return;
        }

        // Check if this chat is already being processed
        let is_processing = processing_chats.read().await.contains(&chat_id);
        if is_processing {
            return;
        }

        tracing::info!(
            chat_id = chat_id,
            trigger_reason = ?trigger_reason,
            "triggering AI completion"
        );

        // Mark chat as processing
        processing_chats.write().await.insert(chat_id);

        // Handle AI request
        let messages_clone = chat_state_clone.messages.clone();
        let trigger_reason_clone = trigger_reason.clone();
        let known_version = chat_state_clone.version;
        let processing_chats_clone2 = Arc::clone(&processing_chats);

        let result = ai_request::handle_ai_request(
            client,
            plugins_monitor,
            ai_client,
            &config,
            &plugin_id,
            chat_id,
            &messages_clone,
            trigger_reason_clone,
            Some(known_version),
        )
        .await;

        // Remove chat from processing set when done
        processing_chats_clone2.write().await.remove(&chat_id);

        if let Err(e) = result {
            tracing::error!("AI request failed for chat {}: {}", chat_id, e);
        }
    });
}).await;

// Keep the plugin running (no more polling loop)
tracing::info!("Plugin running in event-driven mode");
loop {
    tokio::time::sleep(Duration::from_secs(60)).await;
    // Just keep the process alive; all work happens in callbacks
}
```

**Keep startup reconciliation** (lines 108-110, 235-280) — this is still needed to handle chats that existed before the plugin started or were left in an inconsistent state by a crash.

### Phase 3: Refactor rhd_plugin_system_prompt

**File:** `plugins/rhd_plugin_system_prompt/src/plugin.rs`

**Remove the polling loop** (lines 253-281) and replace with event-driven logic:

```rust
// Register callback for chat state changes
let client_for_callback = Arc::clone(&client);
let cached_prompts_for_callback = cached_prompts.clone();

chat_monitor.on_chat_state_change(move |chat_id, chat_state| {
    let client = Arc::clone(&client_for_callback);
    let cached_prompts = cached_prompts_for_callback.clone();
    let chat_state_clone = chat_state.clone();

    tokio::spawn(async move {
        match system_prompt::process_chat(&client, &chat_state_clone, &cached_prompts).await {
            Ok(count) if count > 0 => {
                tracing::info!(
                    chat_id = chat_id,
                    injected_count = count,
                    "injected system prompts after state change"
                );
            }
            Ok(_) => {}
            Err(e) => {
                tracing::error!(
                    chat_id = chat_id,
                    error = %e,
                    "failed to process chat after state change"
                );
            }
        }
    });
}).await;

// Keep the plugin running (no more polling loop)
tracing::info!("Plugin running in event-driven mode");
loop {
    tokio::time::sleep(Duration::from_secs(60)).await;
}
```

**Keep startup reconciliation** (lines 245-250, 288-313) — still needed for existing chats.

**Keep custom event handler** (lines 86-241) — this is already event-driven and handles coordination with ai_completions plugin.

### Phase 4: Update plugins/README.md

**File:** `plugins/README.md`

Add a new section documenting the event-driven design pattern:

```markdown
## Event-Driven Design

Plugins should use event-driven triggers instead of polling loops. The `ChatMonitor` provides a callback mechanism to react to chat state changes:

### Recommended Pattern

```rust
// Register callback for chat state changes
chat_monitor.on_chat_state_change(move |chat_id, chat_state| {
    // Check trigger conditions for THIS chat only
    if should_trigger(&chat_state) {
        // Handle the trigger
    }
}).await;

// Keep the plugin running
loop {
    tokio::time::sleep(Duration::from_secs(60)).await;
}
```

### Why Event-Driven?

- **Efficiency**: Only check chats that actually changed, not all chats
- **Scalability**: O(1) work per event instead of O(n) work per second
- **Lower latency**: React immediately to events instead of waiting for next poll cycle
- **Resource usage**: Reduced CPU and network traffic

### When to Use Startup Reconciliation

Startup reconciliation (checking all chats once on startup) is still needed to:
- Handle chats that existed before the plugin started
- Recover from crashes or inconsistent states
- Process chats that may have been missed during disconnection

### Avoid Polling Loops

**DO NOT** use this pattern:

```rust
// BAD: Polling all chats every second
loop {
    for chat_id in chat_monitor.get_chat_ids().await {
        // Check conditions for EVERY chat
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
}
```

This pattern doesn't scale and wastes resources checking chats that haven't changed.
```

### Phase 5: Add Filtered Message Query API

**Files to modify:**
- `packages/rhd_chat_api/src/methods/get_messages.rs` (new file)
- `packages/rhd_chat_api/src/methods/mod.rs`
- `packages/rhd_chat_server/src/handlers/message.rs`
- `packages/rhd_chat_client/src/client.rs`

**New API method: `get_messages`**

Add a generic method to query messages with filters:

```rust
// In rhd_chat_api/src/methods/get_messages.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMessagesParams {
    pub chat_id: i64,
    /// If true, return only messages with unresolved tool calls
    pub with_unresolved_tool_calls: bool,
    /// Return only messages that have ALL of these tags
    pub with_all_tags: Vec<String>,
    /// Return only messages that have AT LEAST ONE of these tags
    pub with_any_tag: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMessagesResult {
    pub messages: Vec<Message>,
    /// Chat version at the time of query. Plugin can compare with cached version
    /// to verify state consistency.
    pub chat_version: i64,
}
```

**Server implementation** (`rhd_chat_server/src/handlers/message.rs`):

```rust
pub async fn handle_get_messages(
    db: Arc<ChatDb>,
    params: GetMessagesParams,
) -> Result<GetMessagesResult, ServerError> {
    // Get all messages for the chat
    let all_messages = db.get_messages(params.chat_id)?;
    
    // Apply filters
    let mut filtered_messages = all_messages;
    
    if params.with_unresolved_tool_calls {
        filtered_messages = filtered_messages
            .into_iter()
            .filter(|m| has_unresolved_tool_calls(m))
            .collect();
    }
    
    if !params.with_all_tags.is_empty() {
        filtered_messages = filtered_messages
            .into_iter()
            .filter(|m| {
                params.with_all_tags.iter().all(|tag| {
                    m.tags.as_ref().map_or(false, |tags| tags.contains(tag))
                })
            })
            .collect();
    }
    
    if !params.with_any_tag.is_empty() {
        filtered_messages = filtered_messages
            .into_iter()
            .filter(|m| {
                m.tags.as_ref().map_or(false, |tags| {
                    params.with_any_tag.iter().any(|tag| tags.contains(tag))
                })
            })
            .collect();
    }
    
    // Get current chat version
    let chat_version = db.get_chat_version(params.chat_id)?;
    
    Ok(GetMessagesResult {
        messages: filtered_messages,
        chat_version,
    })
}
```

**Client method** (`rhd_chat_client/src/client.rs`):

```rust
pub async fn get_messages(&self, params: GetMessagesParams) -> Result<GetMessagesResult, ClientError> {
    // Implementation similar to other query methods
}
```

**Use cases:**

1. **ai_completions plugin**: Check for unresolved tool calls without loading all messages
   ```rust
   let result = client.get_messages(GetMessagesParams {
       chat_id,
       with_unresolved_tool_calls: true,
       with_all_tags: vec![],
       with_any_tag: vec![],
   }).await?;
   
   if result.messages.is_empty() && result.chat_version == cached_version {
       // No unresolved tool calls, safe to trigger
   }
   ```

2. **system_prompt plugin**: Check if system prompt already exists without loading all messages
   ```rust
   let result = client.get_messages(GetMessagesParams {
       chat_id,
       with_unresolved_tool_calls: false,
       with_all_tags: vec!["system_prompt:my-prompt".to_string()],
       with_any_tag: vec![],
   }).await?;
   
   if result.messages.is_empty() {
       // System prompt not yet injected
   }
   ```

**Benefits:**
- Reduces memory usage by not loading all messages when only checking specific conditions
- `chat_version` in response allows plugins to verify state consistency
- Flexible filtering supports multiple use cases
- Can be extended with more filters in the future

### Phase 6: Identify Cases for New Events

**Analysis of current event usage:**

Both plugins currently fetch full chat state on every event via `get_chat()`. This is necessary because:
- Trigger conditions depend on multiple fields (messages, queue count, tags)
- Events don't carry all the data needed to evaluate triggers

**Potential optimizations considered:**

1. **New event: `chat_tags_updated`** (for both plugins):
   - Currently: Tags are updated via `ChatUpdated` event from chats list
   - Could add: Dedicated event for tag changes
   - Benefit: More granular event, easier to filter
   - **Recommendation**: Not needed. `ChatUpdated` already provides tag changes. Plugins can check if tags changed by comparing with previous state.

2. **New event: `queue_message_promoted`** (for ai_completions):
   - Currently: Queue message added → plugin processes it → adds as regular message
   - Could add: Event when queue message is automatically promoted
   - Benefit: Reduce duplicate work
   - **Recommendation**: Not needed. Current flow is clear and works correctly.

**Conclusion**: No new events are needed at this time. The existing event model combined with the new `get_messages` filtered query API is sufficient. The main improvements are:
1. Switching from polling to event-driven callbacks
2. Using filtered queries to reduce data transfer when checking specific conditions

## Testing Strategy

1. **Unit tests**: Verify `on_chat_state_change` callback registration and invocation
2. **Integration tests**: 
   - Verify ai_completions triggers on queue message added
   - Verify ai_completions triggers on tool call resolution
   - Verify system_prompt injects on new chat creation
   - Verify system_prompt injects on new user message
3. **Performance tests**: 
   - Monitor CPU usage with 100+ chats
   - Verify no polling loops in production (check logs)

## Migration Notes

- Startup reconciliation is still needed and should be kept
- The `processing_chats` set in ai_completions is still needed to prevent duplicate triggers
- Custom event handlers in system_prompt are already event-driven and should be kept
- The main loop should be replaced with a simple keep-alive loop (or removed if the plugin has other async tasks)

## Files to Modify

1. `packages/rhd_chat_api/src/methods/get_messages.rs` — New file: Define `GetMessagesParams` and `GetMessagesResult`
2. `packages/rhd_chat_api/src/methods/mod.rs` — Add `get_messages` module
3. `packages/rhd_chat_server/src/handlers/message.rs` — Implement `handle_get_messages`
4. `packages/rhd_chat_client/src/client.rs` — Add `get_messages` client method
5. `packages/rhd_chat_client/src/chat_monitor.rs` — Add callback mechanism
6. `plugins/rhd_plugin_ai_completions/src/plugin.rs` — Remove polling, add callback
7. `plugins/rhd_plugin_system_prompt/src/plugin.rs` — Remove polling, add callback
8. `plugins/README.md` — Add event-driven design guideline

## Success Criteria

- No polling loops in plugin code (no `tokio::time::sleep(Duration::from_secs(1))` in main loops)
- Plugins react to events within milliseconds, not seconds
- CPU usage remains low even with 100+ chats
- All existing functionality preserved (startup reconciliation, custom event coordination, etc.)
