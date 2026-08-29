# Phase 3: Server Logic Implementation

## Overview

This phase updates the server-side logic to handle the new context fields and rejection flag throughout the custom event lifecycle: creation, broadcasting, acknowledgment, and recovery.

**Scope**: Server-side custom event handling in `rhd_chat_server` crate
**Out of scope**: Database layer, API types, client implementations

## Files to Modify

### 1. `packages/rhd_chat_server/src/custom_events.rs`

**Modify `create_custom_event` function** (lines 15-39):

```rust
/// Create and store a custom event, returning the event and its ID.
pub fn create_custom_event(
    db: &ChatDb,
    event_name: &str,
    sender_plugin_id: Option<&str>,
    additional: Option<&str>,
    chat_id: Option<&str>,
    message_id: Option<&str>,
    tool_call_id: Option<&str>,
) -> Result<(String, Event), ServerError> {
    let event_id = Uuid::new_v4().to_string();
    let created_at = Utc::now();

    // Store in database
    db.create_custom_event(
        &event_id,
        event_name,
        sender_plugin_id,
        additional,
        chat_id,
        message_id,
        tool_call_id,
    )?;

    // Create event for broadcasting
    let data = CustomEventData {
        event_id: event_id.clone(),
        event_name: event_name.to_string(),
        sender_plugin_id: sender_plugin_id.map(|s| s.to_string()),
        additional: additional.map(|s| s.to_string()),
        chat_id: chat_id.map(|s| s.to_string()),
        message_id: message_id.map(|s| s.to_string()),
        tool_call_id: tool_call_id.map(|s| s.to_string()),
        created_at,
    };

    let event = Event::new("customEvent", serde_json::to_value(data)?);

    Ok((event_id, event))
}
```

**Modify `ack_custom_event` function** (lines 42-68):

```rust
/// Acknowledge a custom event and create the acknowledgment event.
pub fn ack_custom_event(
    db: &ChatDb,
    event_id: &str,
    plugin_id: &str,
    is_rejected: bool,
) -> Result<Option<Event>, ServerError> {
    // Check if event exists
    let event_info = db.get_custom_event(event_id)?;
    let event_info = match event_info {
        Some(e) => e,
        None => return Ok(None),
    };

    // Store acknowledgment
    db.ack_custom_event(event_id, plugin_id)?;

    // Create acknowledgment event for the sender
    if let Some(_sender_plugin_id) = &event_info.sender_plugin_id {
        let data = CustomEventAcknowledgedData {
            event_id: event_id.to_string(),
            acknowledging_plugin_id: plugin_id.to_string(),
            is_rejected,
        };
        let event = Event::new("customEventAcknowledged", serde_json::to_value(data)?);
        Ok(Some(event))
    } else {
        Ok(None)
    }
}
```

**Modify `get_pending_events` function** (lines 71-90):

```rust
/// Get pending events for a plugin and convert to API types.
pub fn get_pending_events(
    db: &ChatDb,
    plugin_id: &str,
) -> Result<Vec<PendingEvent>, ServerError> {
    let events = db.get_pending_events_for_plugin(plugin_id)?;
    let pending_events = events
        .into_iter()
        .map(|e| {
            let created_at = e.created_at.parse().unwrap_or_else(|_| Utc::now());
            PendingEvent {
                event_id: e.event_id,
                event_name: e.event_name,
                sender_plugin_id: e.sender_plugin_id,
                additional: e.additional,
                chat_id: e.chat_id,
                message_id: e.message_id,
                tool_call_id: e.tool_call_id,
                created_at,
            }
        })
        .collect();
    Ok(pending_events)
}
```

**Update test** (lines 98-115):

```rust
#[test]
fn test_create_custom_event() {
    let db = ChatDb::new(":memory:").unwrap();
    
    let (event_id, event) = create_custom_event(
        &db,
        "test-event",
        Some("plugin-1"),
        Some("{\"key\": \"value\"}"),
        Some("chat-123"),
        Some("message-456"),
        Some("call-789"),
    ).unwrap();
    
    assert!(!event_id.is_empty());
    assert_eq!(event.event, "customEvent");
    
    // Verify event was stored
    let stored = db.get_custom_event(&event_id).unwrap().unwrap();
    assert_eq!(stored.event_name, "test-event");
    assert_eq!(stored.sender_plugin_id, Some("plugin-1".to_string()));
    assert_eq!(stored.chat_id, Some("chat-123".to_string()));
    assert_eq!(stored.message_id, Some("message-456".to_string()));
    assert_eq!(stored.tool_call_id, Some("call-789".to_string()));
}
```

**Update test** (lines 118-131):

```rust
#[test]
fn test_ack_custom_event() {
    let db = ChatDb::new(":memory:").unwrap();
    db.register_plugin("sender").unwrap();
    db.register_plugin("receiver").unwrap();
    
    let (event_id, _) = create_custom_event(
        &db,
        "test-event",
        Some("sender"),
        None,
        None,
        None,
        None,
    ).unwrap();
    
    let ack_event = ack_custom_event(&db, &event_id, "receiver", false).unwrap();
    assert!(ack_event.is_some());
    assert_eq!(ack_event.unwrap().event, "customEventAcknowledged");
    
    // Verify acknowledgment was stored
    assert!(db.has_plugin_acked(&event_id, "receiver").unwrap());
}
```

**Add new test** (after line 131):

```rust
#[test]
fn test_ack_custom_event_with_rejection() {
    let db = ChatDb::new(":memory:").unwrap();
    db.register_plugin("sender").unwrap();
    db.register_plugin("receiver").unwrap();
    
    let (event_id, _) = create_custom_event(
        &db,
        "test-event",
        Some("sender"),
        None,
        None,
        None,
        None,
    ).unwrap();
    
    // Acknowledge with rejection
    let ack_event = ack_custom_event(&db, &event_id, "receiver", true).unwrap();
    assert!(ack_event.is_some());
    
    let event = ack_event.unwrap();
    assert_eq!(event.event, "customEventAcknowledged");
    
    // Verify the event data contains is_rejected: true
    let data: CustomEventAcknowledgedData = serde_json::from_value(event.data).unwrap();
    assert_eq!(data.event_id, event_id);
    assert_eq!(data.acknowledging_plugin_id, "receiver");
    assert!(data.is_rejected);
    
    // Verify acknowledgment was stored
    assert!(db.has_plugin_acked(&event_id, "receiver").unwrap());
}

#[test]
fn test_create_custom_event_without_context_fields() {
    let db = ChatDb::new(":memory:").unwrap();
    
    let (event_id, event) = create_custom_event(
        &db,
        "test-event",
        Some("plugin-1"),
        Some("{\"key\": \"value\"}"),
        None,
        None,
        None,
    ).unwrap();
    
    assert!(!event_id.is_empty());
    assert_eq!(event.event, "customEvent");
    
    // Verify event was stored with None context fields
    let stored = db.get_custom_event(&event_id).unwrap().unwrap();
    assert_eq!(stored.event_name, "test-event");
    assert_eq!(stored.chat_id, None);
    assert_eq!(stored.message_id, None);
    assert_eq!(stored.tool_call_id, None);
}
```

**Why**: Updates the server logic to accept and pass through the new context fields and rejection flag. The server doesn't validate the fields; it simply stores and forwards them.

### 2. `packages/rhd_chat_server/src/handlers/plugin.rs`

**Modify `send_custom_event` handler** (lines 167-209):

```rust
/// Handle `sendCustomEvent` request.
pub async fn send_custom_event(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    plugin_registry: SharedPluginRegistry,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: SendCustomEventParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Find sender plugin_id (if this connection has a registered plugin)
    let sender_plugin_id = {
        let registry = plugin_registry.read().await;
        let plugins = registry.get_plugins_for_connection(connection_id);
        plugins.into_iter().next()
    };

    // Create and store event
    let (event_id, event) = custom_events::create_custom_event(
        db,
        &params.event_name,
        sender_plugin_id.as_deref(),
        params.additional.as_deref(),
        params.chat_id.as_deref(),
        params.message_id.as_deref(),
        params.tool_call_id.as_deref(),
    )?;

    // Broadcast to ALL connected clients
    let manager = subscription_manager.read().await;
    manager.broadcast_to_all(event);

    let result = SendCustomEventResult { event_id };
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}
```

**Modify `ack_custom_event` handler** (lines 212-274):

```rust
/// Handle `ackCustomEvent` request.
pub async fn ack_custom_event(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    plugin_registry: SharedPluginRegistry,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: AckCustomEventParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Find the plugin_id for this connection
    let plugin_id = {
        let registry = plugin_registry.read().await;
        let plugins = registry.get_plugins_for_connection(connection_id);
        match plugins.into_iter().next() {
            Some(p) => p,
            None => {
                return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                    request_id,
                    "Connection has no registered plugin",
                ))?);
            }
        }
    };

    // Determine rejection status (default to false if not provided)
    let is_rejected = params.is_rejected.unwrap_or(false);

    // Acknowledge event
    let ack_event = custom_events::ack_custom_event(db, &params.event_id, &plugin_id, is_rejected)?;

    // Send acknowledgment to the original sender
    if let Some(event) = ack_event {
        // Find the sender's connection
        let sender_connection_id = {
            let registry = plugin_registry.read().await;
            // Get the event to find sender
            if let Some(event_info) = db.get_custom_event(&params.event_id)? {
                event_info
                    .sender_plugin_id
                    .and_then(|sender_id| registry.get_connection_for_plugin(&sender_id).map(|s| s.to_string()))
            } else {
                None
            }
        };

        if let Some(sender_conn_id) = sender_connection_id {
            let manager = subscription_manager.read().await;
            manager.send_to_connection(&sender_conn_id, event);
        }
    }

    let result = AckCustomEventResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}
```

**Why**: Updates the handlers to extract the new fields from the request parameters and pass them to the custom event functions. The rejection flag defaults to false when not provided.

## Implementation Notes

1. **Pass-through logic**: The server doesn't validate or transform the context fields; it simply stores and forwards them
2. **Rejection doesn't block**: When a plugin rejects an event, the server still processes the acknowledgment and broadcasts it; other plugins can still acknowledge normally
3. **Single acknowledgment per plugin**: Existing logic prevents duplicate acknowledgments; this remains unchanged
4. **Recovery includes context**: `get_pending_events()` returns all fields so plugins can reconstruct the full event context after reconnection
5. **Default rejection value**: When `is_rejected` is not provided in the request, it defaults to `false` (acceptance)

## Dependencies

- **Phase 1** - Database layer must support the new fields
- **Phase 2** - API types must be defined before server can use them
- This phase must be completed before Phase 4 (Integration Tests)

## Success Criteria

- ✅ `sendCustomEvent` accepts and stores context fields
- ✅ `customEvent` broadcast includes context fields
- ✅ `ackCustomEvent` accepts `is_rejected` parameter
- ✅ `customEventAcknowledged` includes `is_rejected` field
- ✅ `getPendingAcks` returns events with all context fields
- ✅ Existing functionality remains unchanged (backward compatible)
- ✅ Rejection by one plugin doesn't prevent others from acknowledging
- ✅ Unit tests verify context fields are passed through correctly
- ✅ Unit tests verify rejection flag is handled correctly
