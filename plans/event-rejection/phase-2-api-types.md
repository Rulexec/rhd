# Phase 2: API Types Enhancement

## Overview

This phase updates all API type definitions to include the new context fields (`chat_id`, `message_id`, `tool_call_id`) and rejection flag (`is_rejected`). This phase defines the contract between clients and the server.

**Scope**: API type definitions in `rhd_chat_api` crate
**Out of scope**: Database layer, server logic, client implementations

## Files to Modify

### 1. `packages/rhd_chat_api/src/methods/send_custom_event.rs`

**Modify `SendCustomEventParams` struct** (lines 16-24):

```rust
/// Parameters for the `sendCustomEvent` method.
///
/// # Example JSON
/// ```json
/// {
///   "eventName": "my-custom-event",
///   "additional": "{\"key\": \"value\"}",
///   "chatId": "chat-123",
///   "messageId": "message-456",
///   "toolCallId": "call-789"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SendCustomEventParams {
    /// Name of the custom event.
    pub event_name: String,
    /// Optional additional JSON data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional: Option<String>,
    /// Optional chat ID for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    /// Optional message ID for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    /// Optional tool call ID for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}
```

**Update test** (lines 46-58):

```rust
#[test]
fn test_send_custom_event_params_serialization() {
    let params = SendCustomEventParams {
        event_name: "my-custom-event".to_string(),
        additional: Some("{\"key\": \"value\"}".to_string()),
        chat_id: Some("chat-123".to_string()),
        message_id: Some("message-456".to_string()),
        tool_call_id: Some("call-789".to_string()),
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"eventName\":\"my-custom-event\""));
    assert!(json.contains("\"additional\""));
    assert!(json.contains("\"chatId\":\"chat-123\""));
    assert!(json.contains("\"messageId\":\"message-456\""));
    assert!(json.contains("\"toolCallId\":\"call-789\""));

    let deserialized: SendCustomEventParams = serde_json::from_str(&json).unwrap();
    assert_eq!(params, deserialized);
}
```

**Add new test** (after line 74):

```rust
#[test]
fn test_send_custom_event_params_with_context_fields() {
    let params = SendCustomEventParams {
        event_name: "my-custom-event".to_string(),
        additional: None,
        chat_id: Some("chat-123".to_string()),
        message_id: Some("message-456".to_string()),
        tool_call_id: Some("call-789".to_string()),
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"chatId\":\"chat-123\""));
    assert!(json.contains("\"messageId\":\"message-456\""));
    assert!(json.contains("\"toolCallId\":\"call-789\""));

    let deserialized: SendCustomEventParams = serde_json::from_str(&json).unwrap();
    assert_eq!(params, deserialized);
}

#[test]
fn test_send_custom_event_params_without_context_fields() {
    let params = SendCustomEventParams {
        event_name: "my-custom-event".to_string(),
        additional: None,
        chat_id: None,
        message_id: None,
        tool_call_id: None,
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"eventName\":\"my-custom-event\""));
    // Context fields are None, should be skipped
    assert!(!json.contains("chatId"));
    assert!(!json.contains("messageId"));
    assert!(!json.contains("toolCallId"));

    let deserialized: SendCustomEventParams = serde_json::from_str(&json).unwrap();
    assert_eq!(params, deserialized);
}
```

**Why**: Adds optional context fields to the send method parameters. Fields are skipped in JSON when None for backward compatibility.

### 2. `packages/rhd_chat_api/src/events/custom_event.rs`

**Modify `CustomEventData` struct** (lines 20-35):

```rust
/// Data payload for the `customEvent` event.
///
/// # Example JSON
/// ```json
/// {
///   "eventId": "generated-uuid-string",
///   "eventName": "my-custom-event",
///   "senderPluginId": "sender-plugin-id",
///   "additional": "{\"key\": \"value\"}",
///   "chatId": "chat-123",
///   "messageId": "message-456",
///   "toolCallId": "call-789",
///   "createdAt": "2026-08-20T18:00:00Z"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomEventData {
    /// Unique identifier for the event.
    pub event_id: String,
    /// Name of the custom event.
    pub event_name: String,
    /// ID of the plugin that sent the event (if sent by a plugin).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_plugin_id: Option<String>,
    /// Optional additional JSON data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional: Option<String>,
    /// Optional chat ID for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    /// Optional message ID for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    /// Optional tool call ID for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Timestamp when the event was created.
    pub created_at: DateTime<Utc>,
}
```

**Update test** (lines 42-60):

```rust
#[test]
fn test_custom_event_data_serialization() {
    let data = CustomEventData {
        event_id: "test-event-id".to_string(),
        event_name: "my-custom-event".to_string(),
        sender_plugin_id: Some("sender-plugin".to_string()),
        additional: Some("{\"key\": \"value\"}".to_string()),
        chat_id: Some("chat-123".to_string()),
        message_id: Some("message-456".to_string()),
        tool_call_id: Some("call-789".to_string()),
        created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
    };

    let json = serde_json::to_string(&data).unwrap();
    assert!(json.contains("\"eventId\":\"test-event-id\""));
    assert!(json.contains("\"eventName\":\"my-custom-event\""));
    assert!(json.contains("\"senderPluginId\":\"sender-plugin\""));
    assert!(json.contains("\"additional\""));
    assert!(json.contains("\"chatId\":\"chat-123\""));
    assert!(json.contains("\"messageId\":\"message-456\""));
    assert!(json.contains("\"toolCallId\":\"call-789\""));
    assert!(json.contains("\"createdAt\""));

    let deserialized: CustomEventData = serde_json::from_str(&json).unwrap();
    assert_eq!(data, deserialized);
}
```

**Update test** (lines 63-80):

```rust
#[test]
fn test_custom_event_data_without_optional_fields() {
    let data = CustomEventData {
        event_id: "test-event-id".to_string(),
        event_name: "my-custom-event".to_string(),
        sender_plugin_id: None,
        additional: None,
        chat_id: None,
        message_id: None,
        tool_call_id: None,
        created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
    };

    let json = serde_json::to_string(&data).unwrap();
    assert!(json.contains("\"eventId\":\"test-event-id\""));
    // Optional fields should be skipped
    assert!(!json.contains("senderPluginId"));
    assert!(!json.contains("additional"));
    assert!(!json.contains("chatId"));
    assert!(!json.contains("messageId"));
    assert!(!json.contains("toolCallId"));

    let deserialized: CustomEventData = serde_json::from_str(&json).unwrap();
    assert_eq!(data, deserialized);
}
```

**Why**: Adds optional context fields to the broadcast event data. Fields are skipped in JSON when None for backward compatibility.

### 3. `packages/rhd_chat_api/src/methods/ack_custom_event.rs`

**Modify `AckCustomEventParams` struct** (lines 15-20):

```rust
/// Parameters for the `ackCustomEvent` method.
///
/// # Example JSON
/// ```json
/// {
///   "eventId": "generated-uuid-string",
///   "isRejected": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AckCustomEventParams {
    /// Unique identifier for the event to acknowledge.
    pub event_id: String,
    /// Whether the event is rejected (defaults to false if not provided).
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_rejected: Option<bool>,
}
```

**Update test** (lines 37-47):

```rust
#[test]
fn test_ack_custom_event_params_serialization() {
    let params = AckCustomEventParams {
        event_id: "test-event-id".to_string(),
        is_rejected: Some(true),
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"eventId\":\"test-event-id\""));
    assert!(json.contains("\"isRejected\":true"));

    let deserialized: AckCustomEventParams = serde_json::from_str(&json).unwrap();
    assert_eq!(params, deserialized);
}
```

**Add new test** (after line 47):

```rust
#[test]
fn test_ack_custom_event_params_with_rejection() {
    let params = AckCustomEventParams {
        event_id: "test-event-id".to_string(),
        is_rejected: Some(true),
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"isRejected\":true"));

    let deserialized: AckCustomEventParams = serde_json::from_str(&json).unwrap();
    assert_eq!(params, deserialized);
}

#[test]
fn test_ack_custom_event_params_without_rejection() {
    let params = AckCustomEventParams {
        event_id: "test-event-id".to_string(),
        is_rejected: None,
    };

    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("\"eventId\":\"test-event-id\""));
    // is_rejected is None, should be skipped
    assert!(!json.contains("isRejected"));

    let deserialized: AckCustomEventParams = serde_json::from_str(&json).unwrap();
    assert_eq!(params, deserialized);
}

#[test]
fn test_ack_custom_event_params_default_rejection() {
    // Test that missing is_rejected field defaults to None
    let json = r#"{"eventId":"test-event-id"}"#;
    let params: AckCustomEventParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.event_id, "test-event-id");
    assert_eq!(params.is_rejected, None);
}
```

**Why**: Adds optional rejection flag to the acknowledgment method. Defaults to None (which means acceptance) for backward compatibility.

### 4. `packages/rhd_chat_api/src/events/custom_event_acknowledged.rs`

**Modify `CustomEventAcknowledgedData` struct** (lines 16-23):

```rust
/// Data payload for the `customEventAcknowledged` event.
///
/// # Example JSON
/// ```json
/// {
///   "eventId": "generated-uuid-string",
///   "acknowledgingPluginId": "acknowledging-plugin-id",
///   "isRejected": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomEventAcknowledgedData {
    /// Unique identifier for the event that was acknowledged.
    pub event_id: String,
    /// ID of the plugin that acknowledged the event.
    pub acknowledging_plugin_id: String,
    /// Whether the event was rejected (false means accepted).
    #[serde(default)]
    pub is_rejected: bool,
}
```

**Update test** (lines 30-42):

```rust
#[test]
fn test_custom_event_acknowledged_data_serialization() {
    let data = CustomEventAcknowledgedData {
        event_id: "test-event-id".to_string(),
        acknowledging_plugin_id: "ack-plugin".to_string(),
        is_rejected: true,
    };

    let json = serde_json::to_string(&data).unwrap();
    assert!(json.contains("\"eventId\":\"test-event-id\""));
    assert!(json.contains("\"acknowledgingPluginId\":\"ack-plugin\""));
    assert!(json.contains("\"isRejected\":true"));

    let deserialized: CustomEventAcknowledgedData = serde_json::from_str(&json).unwrap();
    assert_eq!(data, deserialized);
}
```

**Add new test** (after line 42):

```rust
#[test]
fn test_custom_event_acknowledged_data_with_acceptance() {
    let data = CustomEventAcknowledgedData {
        event_id: "test-event-id".to_string(),
        acknowledging_plugin_id: "ack-plugin".to_string(),
        is_rejected: false,
    };

    let json = serde_json::to_string(&data).unwrap();
    assert!(json.contains("\"isRejected\":false"));

    let deserialized: CustomEventAcknowledgedData = serde_json::from_str(&json).unwrap();
    assert_eq!(data, deserialized);
}

#[test]
fn test_custom_event_acknowledged_data_default_rejection() {
    // Test that missing is_rejected field defaults to false
    let json = r#"{"eventId":"test-event-id","acknowledgingPluginId":"ack-plugin"}"#;
    let data: CustomEventAcknowledgedData = serde_json::from_str(json).unwrap();
    assert_eq!(data.event_id, "test-event-id");
    assert_eq!(data.acknowledging_plugin_id, "ack-plugin");
    assert_eq!(data.is_rejected, false);
}
```

**Why**: Adds rejection flag to the acknowledgment event. Uses `#[serde(default)]` to default to false when not provided, maintaining backward compatibility.

### 5. `packages/rhd_chat_api/src/common.rs`

**Modify `PendingEvent` struct** (lines 160-173):

```rust
/// A pending custom event that has not been acknowledged.
///
/// # Example JSON
/// ```json
/// {
///   "eventId": "generated-uuid-string",
///   "eventName": "my-custom-event",
///   "senderPluginId": "sender-plugin",
///   "additional": "{\"key\": \"value\"}",
///   "chatId": "chat-123",
///   "messageId": "message-456",
///   "toolCallId": "call-789",
///   "createdAt": "2026-08-20T18:00:00Z"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PendingEvent {
    /// Unique identifier for the event.
    pub event_id: String,
    /// Name of the event.
    pub event_name: String,
    /// ID of the plugin that sent the event.
    pub sender_plugin_id: Option<String>,
    /// Additional JSON data.
    pub additional: Option<String>,
    /// Optional chat ID for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    /// Optional message ID for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    /// Optional tool call ID for context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Timestamp when the event was created.
    pub created_at: DateTime<Utc>,
}
```

**Add new test** (after line 338):

```rust
#[test]
fn test_pending_event_with_context_fields() {
    let event = PendingEvent {
        event_id: "test-event-id".to_string(),
        event_name: "my-event".to_string(),
        sender_plugin_id: Some("sender".to_string()),
        additional: None,
        chat_id: Some("chat-123".to_string()),
        message_id: Some("message-456".to_string()),
        tool_call_id: Some("call-789".to_string()),
        created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"chatId\":\"chat-123\""));
    assert!(json.contains("\"messageId\":\"message-456\""));
    assert!(json.contains("\"toolCallId\":\"call-789\""));

    let deserialized: PendingEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, deserialized);
}

#[test]
fn test_pending_event_without_context_fields() {
    let event = PendingEvent {
        event_id: "test-event-id".to_string(),
        event_name: "my-event".to_string(),
        sender_plugin_id: None,
        additional: None,
        chat_id: None,
        message_id: None,
        tool_call_id: None,
        created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(!json.contains("chatId"));
    assert!(!json.contains("messageId"));
    assert!(!json.contains("toolCallId"));

    let deserialized: PendingEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(event, deserialized);
}
```

**Why**: Adds optional context fields to the pending event structure used for recovery. Fields are skipped in JSON when None for backward compatibility.

## Implementation Notes

1. **Optional fields with skip_serializing_if**: All new context fields use `#[serde(skip_serializing_if = "Option::is_none")]` to maintain backward compatibility
2. **Boolean for rejection**: `is_rejected` is a boolean (not an enum or string) for simplicity
3. **Default behavior**: 
   - In `AckCustomEventParams`: `is_rejected` is `Option<bool>` with `#[serde(default)]`, so missing field = None
   - In `CustomEventAcknowledgedData`: `is_rejected` is `bool` with `#[serde(default)]`, so missing field = false
4. **Consistent naming**: Field names match across all structures (`chat_id`, `message_id`, `tool_call_id`, `is_rejected`)
5. **JSON field names**: All fields use camelCase in JSON (e.g., `chatId`, `messageId`, `toolCallId`, `isRejected`)

## Dependencies

- **Phase 1** - Database must support the new fields before API types can reference them
- This phase must be completed before Phase 3 (Server Logic)

## Success Criteria

- ✅ All API types compile with new fields
- ✅ Serialization/deserialization tests pass for all modified types
- ✅ Optional fields are correctly omitted from JSON when `None`
- ✅ `is_rejected` defaults to `false` in `CustomEventAcknowledgedData` when not provided
- ✅ `is_rejected` defaults to `None` in `AckCustomEventParams` when not provided
- ✅ Existing API contracts remain unchanged (backward compatible)
- ✅ New tests verify context fields serialization/deserialization
- ✅ New tests verify rejection flag behavior
