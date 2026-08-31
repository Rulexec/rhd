# RHD Plugin System

## Overview

The RHD plugin system allows external binary applications to connect to the chat server, register themselves, subscribe to events, and interact with the chat system through a well-defined WebSocket API. Plugins extend the chat system's capabilities by reacting to events and performing automated actions.

## Plugin Lifecycle

Plugins follow a standard lifecycle:

1. **Connect**: Plugin connects to chat server via WebSocket
2. **Register**: Plugin calls `registerPlugin` with unique ID
3. **Get Pending Acks**: Plugin calls `getPendingAcks` to recover missed events
4. **Subscribe**: Plugin subscribes to relevant events (chat list, individual chats, plugins list)
5. **React**: Plugin responds to events based on its logic

## Event Model

### Custom Events

Plugins can emit custom events via `sendCustomEvent`. Custom events have:
- **Event Name**: Unique identifier (e.g., `ai_completions:preRequest`)
- **Sender Plugin ID**: Automatically set to the sending plugin's ID
- **Additional Data**: JSON payload with event-specific data
- **Created At**: Timestamp of event creation

### Acknowledgments

Plugins acknowledge events via `ackCustomEvent`. This is used for coordination between plugins:
- When a plugin emits an event, other plugins can acknowledge it
- The sender can wait for all acknowledgments before proceeding
- Acknowledgments are tracked per-plugin

### Pending Acks

Events that haven't been acknowledged by the current plugin are called "pending acks". On startup, plugins must call `getPendingAcks` to handle events they may have missed while disconnected.

## Tool Call Tags

Assistant messages carry their tool calls in the `toolCalls` array of the `Message` payload (visible in `getChat`, `messageAdded`, and `messageUpdated`). Each tool call has an optional `tags` array that plugins can use for per-call state tracking (e.g., `reviewed`, `failed`, `processed`).

- **Mutate tags**: call `updateToolCallTags` with `{ messageId, toolCallId, addTags, removeTags }`. The server reads the message's stored tool calls, parses them, applies the tag changes to the matching tool call, saves the result, and broadcasts a `messageUpdated` event to all chat subscribers so they can sync their state.
- **Validate freshness**: every `messageUpdated` event carries `chatVersion` — the chat version after the change. Plugins can compare it against their cached version to detect missed events (or refetch via `getChat`).
- **Hazard — full-blob overwrite**: `updateMessage` with `toolCalls` replaces the entire tool-call JSON blob, wiping any tags added via `updateToolCallTags`. Write `toolCalls` only once (at stream finish, as the ai_completions plugin does); always change tags through `updateToolCallTags`.

## Tool Messages

A plugin that executed tools posts results as regular messages:

- `addMessage` with `role: "tool"` **and** `toolCallId` (the id of the assistant tool call being answered). The server rejects `role: "tool"` without `toolCallId`.
- The id is stored immutably and reaches the AI provider in `tool.tool_call_id` on the next request.
- `addQueueMessage` accepts the same optional `toolCallId`; the id survives promotion into the conversation.

Per-call `tags` on assistant `tool_calls` (see "Tool Call Tags") are RHD-internal orchestration metadata: they are **never** sent to the AI provider.

## Plugin Responsibilities

Each plugin should document:
- **Which events it listens for**: Chat events, custom events, etc.
- **What tags it adds to messages/chat**: For state tracking and filtering
- **What events it emits and when**: Custom events for coordination
- **Configuration format**: How to configure the plugin

## Configuration Conventions

- Plugins accept config file path as command-line argument (`--config`)
- Config files use YAML format
- Credentials should be in separate file referenced by config (`credentialsConfig`)
- Use environment variable substitution where appropriate
- Configuration should be validated on startup with clear error messages

## Creating a New Plugin

### Step-by-Step Guide

1. **Create package in `plugins/` directory**
   ```
   plugins/
     my_plugin/
       Cargo.toml
       src/
         main.rs
         config.rs
         plugin.rs
       README.md
   ```

2. **Define `Cargo.toml` with required dependencies**
   ```toml
   [package]
   name = "my_plugin"
   version = "0.1.0"
   edition = "2021"

   [dependencies]
   rhd_chat_client = { path = "../../packages/rhd_chat_client" }
   rhd_chat_api = { path = "../../packages/rhd_chat_api" }
   tokio = { version = "1", features = ["full"] }
   serde = { version = "1", features = ["derive"] }
   serde_yaml = "0.9"
   tracing = "0.1"
   tracing-subscriber = { version = "0.3", features = ["env-filter"] }
   clap = { version = "4", features = ["derive"] }
   thiserror = "1"
   ```

3. **Implement configuration loading**
   - Define config structures with `serde::Deserialize`
   - Implement `load_config()` function
   - Validate configuration on load
   - Handle credentials separately

4. **Implement main plugin logic**
   - Connect to chat server
   - Register as plugin
   - Get and process pending acks
   - Subscribe to relevant events
   - Implement event handlers
   - Keep plugin running

5. **Create README documenting plugin behavior**
   - Trigger conditions
   - Events emitted
   - Tags added
   - Configuration format
   - Dependencies

## Plugin README Template

```markdown
# My Plugin

## Overview
Brief description of what this plugin does.

## Trigger Conditions
When does this plugin activate?

## Events Emitted
List of custom events this plugin emits:
- `event_name`: Description and payload format

## Tags Added
List of tags this plugin adds:
- `tag_name`: When and why this tag is added

## Configuration Format
```yaml
# Example configuration
```

## Dependencies
- **rhd_chat_client**: For connecting to chat server
- **Other plugins**: List any dependencies on other plugins

## Error Handling
How does this plugin handle errors?
```

## Best Practices

1. **Error Handling**: Always handle errors gracefully and log them with `tracing`
2. **Logging**: Use structured logging with appropriate log levels
3. **Configuration Validation**: Validate all configuration on startup
4. **Event Coordination**: Use custom events and acknowledgments for coordination
5. **State Management**: Use tags to track state and prevent duplicate processing
6. **Graceful Shutdown**: Handle shutdown signals gracefully

## Event-Driven Design

Plugins should use event-driven triggers instead of polling loops. The `ChatMonitor` provides a callback mechanism to react to chat state changes.

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

## Custom Event Handling

### Acknowledging Unhandled Events

When a plugin subscribes to custom events via `on_custom_event`, it **must acknowledge all events it receives**, even if it doesn't handle them. This is critical for the coordination system to work correctly.

**Why this matters:**
- The sender of a custom event waits for acknowledgments from all registered plugins
- If a plugin doesn't acknowledge an event, the sender will timeout waiting
- This can block the entire coordination flow

**Correct pattern:**

```rust
client.on_custom_event(move |event| {
    let client = Arc::clone(&client_for_events);
    
    async move {
        let event_id = event.event_id.clone();
        
        // Only handle specific events
        if event.event_name != "my_plugin:specificEvent" {
            // IMPORTANT: Acknowledge events we don't handle
            tracing::debug!(
                event_id = %event_id,
                event_name = %event.event_name,
                "acknowledging unhandled event"
            );
            let _ = client
                .ack_custom_event(AckCustomEventParams {
                    event_id: event_id.clone(),
                    is_rejected: None,
                })
                .await;
            return;
        }
        
        // Handle the event...
        
        // Acknowledge after processing
        let _ = client
            .ack_custom_event(AckCustomEventParams {
                event_id: event_id.clone(),
                is_rejected: None,
            })
            .await;
    }
});
```

**Incorrect pattern (DO NOT USE):**

```rust
client.on_custom_event(move |event| {
    async move {
        // WRONG: Returning without acknowledging unhandled events
        if event.event_name != "my_plugin:specificEvent" {
            return; // This will cause timeouts!
        }
        
        // Handle the event...
    }
});
```

### Best Practices for Custom Events

1. **Always acknowledge**: Every custom event must be acknowledged, whether handled or not
2. **Acknowledge early for unhandled events**: Don't process unhandled events, just acknowledge and return
3. **Acknowledge after processing**: For handled events, acknowledge after completing the work
4. **Handle acknowledgment errors**: Log errors but don't fail the plugin if acknowledgment fails
5. **Use event filtering**: Check event name early to avoid unnecessary processing

## Examples

See `plugins/rhd_plugin_ai_completions/` for a complete example implementation.
