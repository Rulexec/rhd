# Phase 3: rhd_chat_api Stream Methods & Events

## Overview

Define the WebSocket protocol types for stream operations in `rhd_chat_api`: method parameter/result types for `streamPush`, `streamSubscribe`, `streamFinish`, and event data types for `streamChunk` and `streamFinished`.

## Scope

- Create method types: `StreamPushParams`, `StreamPushResult`, `StreamSubscribeParams`, `StreamSubscribeResult`, `StreamFinishParams`, `StreamFinishResult`
- Create event types: `StreamChunkData`, `StreamFinishedData`
- Register new modules in `methods/mod.rs` and `events/mod.rs`
- Add client methods in `rhd_chat_client` for the new stream operations

## Dependencies

- Phase 1 (needs the updated Message types).
- Independent of Phase 2 (can be developed in parallel), but both must be complete before Phase 4.

---

## Files to Create

### 1. `packages/rhd_chat_api/src/methods/stream_push.rs` (NEW)

```rust
//! `streamPush` method types.
//!
//! Push streaming content deltas to an active stream.

use serde::{Deserialize, Serialize};

/// A tool call delta during streaming.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamToolCallDelta {
    /// Unique identifier for the tool call.
    pub id: String,
    /// Tool/function name.
    pub name: String,
    /// Arguments delta (incremental JSON string fragment).
    pub arguments: String,
}

/// Parameters for the `streamPush` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "reasoningContent": "Let me think...",
///   "content": "Hello",
///   "toolCalls": []
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamPushParams {
    /// ID of the chat whose stream to push to.
    pub chat_id: i64,
    /// Reasoning/thinking content delta (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Main content delta (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Tool call deltas (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<StreamToolCallDelta>>,
}

/// Result of the `streamPush` method.
///
/// # Example JSON
/// ```json
/// {
///   "success": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamPushResult {
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_push_params_serialization() {
        let params = StreamPushParams {
            chat_id: 123,
            reasoning_content: Some("Thinking...".to_string()),
            content: Some("Hello".to_string()),
            tool_calls: None,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"reasoningContent\":\"Thinking...\""));
        assert!(json.contains("\"content\":\"Hello\""));
        assert!(!json.contains("toolCalls"));

        let deserialized: StreamPushParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_stream_push_params_minimal() {
        let json = r#"{"chatId":123}"#;
        let params: StreamPushParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.chat_id, 123);
        assert!(params.reasoning_content.is_none());
        assert!(params.content.is_none());
        assert!(params.tool_calls.is_none());
    }

    #[test]
    fn test_stream_push_result_serialization() {
        let result = StreamPushResult { success: true };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
    }
}
```

---

### 2. `packages/rhd_chat_api/src/methods/stream_subscribe.rs` (NEW)

```rust
//! `streamSubscribe` method types.
//!
//! Subscribe to a stream and get current accumulated content.

use serde::{Deserialize, Serialize};

use super::stream_push::StreamToolCallDelta;

/// Parameters for the `streamSubscribe` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamSubscribeParams {
    /// ID of the chat whose stream to subscribe to.
    pub chat_id: i64,
}

/// Result of the `streamSubscribe` method.
///
/// Returns the current accumulated stream state. The caller is also
/// subscribed to future `streamChunk` events for this chat.
///
/// # Example JSON
/// ```json
/// {
///   "reasoningContent": "Let me think...",
///   "content": "Hello world",
///   "toolCalls": [],
///   "isFinished": false
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamSubscribeResult {
    /// Accumulated reasoning/thinking content so far.
    pub reasoning_content: String,
    /// Accumulated main content so far.
    pub content: String,
    /// Accumulated tool calls so far.
    pub tool_calls: Vec<StreamToolCallDelta>,
    /// Whether the stream has already finished.
    pub is_finished: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_subscribe_params_serialization() {
        let params = StreamSubscribeParams { chat_id: 123 };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));

        let deserialized: StreamSubscribeParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_stream_subscribe_result_serialization() {
        let result = StreamSubscribeResult {
            reasoning_content: "Thinking...".to_string(),
            content: "Hello".to_string(),
            tool_calls: vec![],
            is_finished: false,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"reasoningContent\":\"Thinking...\""));
        assert!(json.contains("\"content\":\"Hello\""));
        assert!(json.contains("\"isFinished\":false"));

        let deserialized: StreamSubscribeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
}
```

---

### 3. `packages/rhd_chat_api/src/methods/stream_finish.rs` (NEW)

```rust
//! `streamFinish` method types.
//!
//! Finish an active stream and update the associated message.

use serde::{Deserialize, Serialize};

use super::stream_push::StreamToolCallDelta;

/// Parameters for the `streamFinish` method.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "reasoningContent": "Full reasoning text",
///   "content": "Full content text",
///   "toolCalls": []
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamFinishParams {
    /// ID of the chat whose stream to finish.
    pub chat_id: i64,
    /// Final reasoning content (overrides accumulated deltas).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Final main content (overrides accumulated deltas).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Final tool calls (overrides accumulated deltas).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<StreamToolCallDelta>>,
}

/// Result of the `streamFinish` method.
///
/// # Example JSON
/// ```json
/// {
///   "success": true
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamFinishResult {
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_finish_params_serialization() {
        let params = StreamFinishParams {
            chat_id: 123,
            reasoning_content: Some("Full reasoning".to_string()),
            content: Some("Full content".to_string()),
            tool_calls: None,
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"reasoningContent\":\"Full reasoning\""));
        assert!(json.contains("\"content\":\"Full content\""));

        let deserialized: StreamFinishParams = serde_json::from_str(&json).unwrap();
        assert_eq!(params, deserialized);
    }

    #[test]
    fn test_stream_finish_params_minimal() {
        let json = r#"{"chatId":123}"#;
        let params: StreamFinishParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.chat_id, 123);
        assert!(params.reasoning_content.is_none());
        assert!(params.content.is_none());
        assert!(params.tool_calls.is_none());
    }

    #[test]
    fn test_stream_finish_result_serialization() {
        let result = StreamFinishResult { success: true };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
    }
}
```

---

### 4. `packages/rhd_chat_api/src/events/stream_chunk.rs` (NEW)

```rust
//! `streamChunk` event data.
//!
//! Emitted when new content is pushed to a stream. Sent to all clients
//! subscribed to the chat (via subscribeChat or streamSubscribe).

use serde::{Deserialize, Serialize};

use crate::methods::stream_push::StreamToolCallDelta;

/// Data payload for the `streamChunk` event.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123,
///   "type": "contentDelta",
///   "content": "Hello"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamChunkData {
    /// ID of the chat the stream belongs to.
    pub chat_id: i64,
    /// Type of chunk: "reasoningDelta", "contentDelta", or "toolCallDelta".
    pub chunk_type: String,
    /// Content delta (for reasoningDelta and contentDelta).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Tool call deltas (for toolCallDelta).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<StreamToolCallDelta>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_chunk_content_delta() {
        let data = StreamChunkData {
            chat_id: 123,
            chunk_type: "contentDelta".to_string(),
            content: Some("Hello".to_string()),
            tool_calls: None,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"chatId\":123"));
        assert!(json.contains("\"type\":\"contentDelta\""));
        assert!(json.contains("\"content\":\"Hello\""));
        assert!(!json.contains("toolCalls"));
    }

    #[test]
    fn test_stream_chunk_tool_call_delta() {
        let data = StreamChunkData {
            chat_id: 123,
            chunk_type: "toolCallDelta".to_string(),
            content: None,
            tool_calls: Some(vec![StreamToolCallDelta {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                arguments: "{\"path\":\"/tmp\"}".to_string(),
            }]),
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"type\":\"toolCallDelta\""));
        assert!(json.contains("\"toolCalls\""));
        assert!(!json.contains("\"content\""));
    }
}
```

---

### 5. `packages/rhd_chat_api/src/events/stream_finished.rs` (NEW)

```rust
//! `streamFinished` event data.
//!
//! Emitted when a stream is finished. Sent to all clients subscribed to the chat.

use serde::{Deserialize, Serialize};

/// Data payload for the `streamFinished` event.
///
/// # Example JSON
/// ```json
/// {
///   "chatId": 123
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamFinishedData {
    /// ID of the chat whose stream finished.
    pub chat_id: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_finished_data_serialization() {
        let data = StreamFinishedData { chat_id: 123 };
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"chatId\":123"));

        let deserialized: StreamFinishedData = serde_json::from_str(&json).unwrap();
        assert_eq!(data, deserialized);
    }
}
```

---

## Files to Modify

### 6. `packages/rhd_chat_api/src/methods/mod.rs`

**Add module declarations and re-exports:**

```rust
pub mod stream_finish;
pub mod stream_push;
pub mod stream_subscribe;

pub use stream_finish::*;
pub use stream_push::*;
pub use stream_subscribe::*;
```

---

### 7. `packages/rhd_chat_api/src/events/mod.rs`

**Add module declarations and re-exports:**

```rust
pub mod stream_chunk;
pub mod stream_finished;

pub use stream_chunk::*;
pub use stream_finished::*;
```

---

### 8. `packages/rhd_chat_client/src/client.rs`

**Add client methods for stream operations:**

```rust
use rhd_chat_api::{
    StreamFinishParams, StreamFinishResult, StreamPushParams, StreamPushResult,
    StreamSubscribeParams, StreamSubscribeResult,
};

impl ChatClient {
    /// Push streaming content deltas to a stream.
    pub async fn stream_push(
        &self,
        params: StreamPushParams,
    ) -> Result<StreamPushResult, ClientError> {
        let result = self
            .send_request("streamPush", serde_json::to_value(params)?)
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    /// Subscribe to a stream and get current accumulated content.
    pub async fn stream_subscribe(
        &self,
        params: StreamSubscribeParams,
    ) -> Result<StreamSubscribeResult, ClientError> {
        let result = self
            .send_request("streamSubscribe", serde_json::to_value(params)?)
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    /// Finish a stream.
    pub async fn stream_finish(
        &self,
        params: StreamFinishParams,
    ) -> Result<StreamFinishResult, ClientError> {
        let result = self
            .send_request("streamFinish", serde_json::to_value(params)?)
            .await?;
        Ok(serde_json::from_value(result)?)
    }
}
```

---

## Tests

### Unit Tests

All new files include inline `#[cfg(test)]` modules with serialization/deserialization tests. These verify:

1. **`StreamPushParams`**: Full serialization, minimal (chat_id only), with tool calls.
2. **`StreamSubscribeParams`**: Serialization round-trip.
3. **`StreamSubscribeResult`**: Serialization with all fields.
4. **`StreamFinishParams`**: Full serialization, minimal (chat_id only).
5. **`StreamChunkData`**: Content delta variant, tool call delta variant.
6. **`StreamFinishedData`**: Basic serialization.

---

## Implementation Notes

1. **`StreamToolCallDelta` is shared**: Defined in `stream_push.rs` and reused by `stream_subscribe.rs`, `stream_finish.rs`, and `stream_chunk.rs`. This avoids duplication.

2. **`streamFinish` params are optional overrides**: The `reasoning_content`, `content`, and `tool_calls` fields in `StreamFinishParams` are optional. If provided, they override the accumulated deltas. If not provided, the accumulated values from the stream are used. This allows the plugin to set final values that may differ from streaming deltas (e.g., the AI provider may return final tool calls that differ from streaming deltas).

3. **Event format**: `streamChunk` events use a `type` field to distinguish between `reasoningDelta`, `contentDelta`, and `toolCallDelta`. This is a tagged union pattern that makes it easy for the frontend to handle different chunk types.

4. **Client methods**: The `ChatClient` methods are thin wrappers around `send_request`. They handle serialization/deserialization and error mapping.
