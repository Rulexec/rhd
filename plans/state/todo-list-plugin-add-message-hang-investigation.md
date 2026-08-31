# Todo List Plugin add_message Hang Investigation

## Status
**Resolved** - Root cause identified and fixed

## Problem Description
The `rhd_plugin_todo_list` hangs when calling `add_message` during `ai_completions:preRequest` event handling. This causes the AI completions plugin to timeout waiting for acknowledgments from all plugins.

## Observed Behavior

### Log Sequence
```
[todo_list] 2026-08-31T20:14:41.780504Z  INFO rhd_plugin_todo_list::plugin: received ai_completions:preRequest event event_id=aeded877-e74a-4ea8-ac78-e0f782693039
[todo_list] 2026-08-31T20:14:41.780526Z  WARN rhd_plugin_todo_list::plugin: about to add todo_list:current system message event_id=aeded877-e74a-4ea8-ac78-e0f782693039 chat_id=17
[todo_list] 2026-08-31T20:14:41.780592Z DEBUG rhd_chat_client::client: sending request method="addMessage" request_id=f59938ac-f310-4482-bb6b-30d317f846e2
[todo_list] 2026-08-31T20:14:41.780614Z DEBUG rhd_chat_client::client: registered pending request request_id=f59938ac-f310-4482-bb6b-30d317f846e2 pending_count=1
```

**No further logs from todo_list plugin** - the request never completes.

### Key Observations
1. ✅ The plugin receives the event successfully
2. ✅ The `addMessage` request is sent and registered as pending
3. ✅ The message IS actually added to the chat (visible in UI)
4. ❌ The response never comes back to the client
5. ❌ The plugin hangs and never acknowledges the event
6. ❌ AI completions plugin times out after 30 seconds

### Contrast with Working Code
The first `add_message` call (adding the contract system message during chat initialization) works fine. Only the second call during event handling hangs.

## Potential Root Causes

### 1. Request/Response Matching Issue
- The response might be received but not matched to the pending request
- Could be a bug in request ID tracking or response routing
- **Investigation**: Check `rhd_chat_client::client` response handling logic

### 2. WebSocket Message Loss
- The response might be lost in transit over WebSocket
- Could be a network issue or WebSocket buffer problem
- **Investigation**: Add logging to WebSocket message receive handler

### 3. Concurrent Request Deadlock
- Multiple concurrent requests might cause a deadlock
- The event handler might be blocking the response processing thread
- **Investigation**: Check if event handlers run on the same task as response processing

### 4. Server-Side Issue
- The server might not be sending the response
- Could be a bug in the message handler or response serialization
- **Investigation**: Add server-side logging for addMessage responses

### 5. Async Task Scheduling Issue
- The async task waiting for the response might not be scheduled
- Could be a Tokio runtime issue or task starvation
- **Investigation**: Check task spawning and runtime configuration

## Next Steps for Investigation

### Priority 1: Verify Response Reception
Add logging to `rhd_chat_client::client` to see if the response is received:
```rust
// In the response handling loop
tracing::info!(request_id = %response.id, "received response");
```

### Priority 2: Check Concurrent Request Handling
Verify if the event handler and response processor run on the same task:
- Check how `on_custom_event` callbacks are spawned
- Verify if they share the same WebSocket connection task

### Priority 3: Add Timeout to add_message
Add a timeout to the `add_message` call to prevent indefinite hanging:
```rust
let add_result = tokio::time::timeout(
    Duration::from_secs(5),
    client.add_message(...)
).await;
```

### Priority 4: Compare with Working Code
Diff the working `add_message` call (contract message) with the hanging one:
- Are there any differences in parameters?
- Is the timing different (initialization vs event handling)?
- Are there different locks or state held?

## Files to Investigate
- `packages/rhd_chat_client/src/client.rs` - Request/response handling
- `packages/rhd_chat_server/src/handlers/message.rs` - Server-side message handling
- `plugins/rhd_plugin_todo_list/src/plugin.rs` - Event handler implementation
- `packages/rhd_chat_client/src/event_stream.rs` - WebSocket event processing

## Workaround (Temporary)
If this blocks development, consider:
1. Acknowledge the event BEFORE adding the message (fire-and-forget)
2. Add the message in a separate background task
3. Skip the todo list injection if it times out

## Related Issues
- AI completions timeout waiting for plugin acknowledgments
- Todo list not being injected into AI context

## Root Cause Analysis

**Root Cause Identified**: Async deadlock in event dispatch

The `customEvent` callback in [`packages/rhd_chat_client/src/client.rs`](packages/rhd_chat_client/src/client.rs:474) was being awaited **inline** within the WebSocket read task:

```rust
"customEvent" => {
    if let Ok(data) = serde_json::from_value::<rhd_chat_api::CustomEventData>(event.data.clone()) {
        for sub in &subs.custom_event_subscriptions {
            (sub.callback)(data.clone()).await;  // <-- BLOCKS read task!
        }
    }
}
```

### Deadlock Sequence

1. WebSocket read task receives `customEvent` (ai_completions:preRequest)
2. Read task **awaits** the todo_list plugin's callback
3. Plugin callback calls `client.add_message()` which sends a request and awaits response
4. Server processes the request and sends response back
5. Response arrives at WebSocket but **read task is blocked** waiting for callback to complete
6. **Deadlock**: Response can never be processed because read task is stuck

### Why First add_message Worked

The first `add_message` call (contract message during chat initialization) worked because it was called from a `tokio::spawn`'d task (line 104 in plugin.rs), not from within an event callback. This meant it didn't block the read task.

## Fix Applied

Changed all event callbacks that were awaited inline to spawn separate tasks:

**Before** (blocking):
```rust
(sub.callback)(data.clone()).await;
```

**After** (non-blocking):
```rust
let callback = sub.callback.clone();
let data_clone = data.clone();
tokio::spawn(async move {
    (callback)(data_clone).await;
});
```

### Affected Event Types

Fixed in [`packages/rhd_chat_client/src/client.rs`](packages/rhd_chat_client/src/client.rs:453-486):
- `pluginRegistered`
- `pluginUpdated`
- `pluginRemoved`
- `customEvent` ← **Primary fix for this issue**
- `customEventAcknowledged`

This matches the pattern already used for other events like `messageAdded`, `chatCreated`, etc.

## Verification

- ✅ Code compiles successfully
- ✅ All tests pass
- ✅ Consistent with existing event dispatch patterns in the codebase
