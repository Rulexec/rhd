//! Protocol envelope types for the chat WebSocket protocol.
//!
//! This module defines the base message envelope types used for
//! all communication over the WebSocket connection:
//! - [`Request`] — Client → Server requests
//! - [`Response`] — Server → Client responses (success or error)
//! - [`Event`] — Server → Client event notifications

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A request message sent from client to server.
///
/// # Example JSON
/// ```json
/// {
///   "type": "request",
///   "id": "uuid-string",
///   "method": "createChat",
///   "params": {
///     "title": "Chat Title",
///     "tags": ["tag1", "tag2"]
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    /// Always "request".
    pub r#type: String,
    /// Unique request identifier (UUID string).
    pub id: String,
    /// Method name to invoke.
    pub method: String,
    /// Method parameters (method-specific).
    #[serde(default)]
    pub params: Value,
}

impl Request {
    /// Create a new request with the given method and parameters.
    pub fn new(id: impl Into<String>, method: impl Into<String>, params: Value) -> Self {
        Request {
            r#type: "request".to_string(),
            id: id.into(),
            method: method.into(),
            params,
        }
    }
}

/// A successful response message sent from server to client.
///
/// # Example JSON
/// ```json
/// {
///   "type": "response",
///   "id": "uuid-string",
///   "success": true,
///   "data": {
///     "chatId": 123
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    /// Always "response".
    pub r#type: String,
    /// Request ID this response corresponds to.
    pub id: String,
    /// Always true for successful responses.
    pub success: bool,
    /// Method-specific result data.
    pub data: Value,
}

impl Response {
    /// Create a new successful response with the given data.
    pub fn success(id: impl Into<String>, data: Value) -> Self {
        Response {
            r#type: "response".to_string(),
            id: id.into(),
            success: true,
            data,
        }
    }
}

/// An event message pushed from server to subscribed clients.
///
/// # Example JSON
/// ```json
/// {
///   "type": "event",
///   "event": "chatCreated",
///   "data": {
///     "chat": {
///       "id": 123,
///       "title": "New Chat",
///       "createdAt": "2026-08-20T18:00:00Z",
///       "updatedAt": "2026-08-20T18:00:00Z",
///       "tags": ["tag1"]
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// Always "event".
    pub r#type: String,
    /// Event name (e.g., "chatCreated", "messageAdded").
    pub event: String,
    /// Event-specific data payload.
    pub data: Value,
}

impl Event {
    /// Create a new event with the given name and data.
    pub fn new(event: impl Into<String>, data: Value) -> Self {
        Event {
            r#type: "event".to_string(),
            event: event.into(),
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let request = Request::new(
            "test-id",
            "createChat",
            serde_json::json!({
                "title": "Test Chat",
                "tags": ["tag1"]
            }),
        );

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"type\":\"request\""));
        assert!(json.contains("\"id\":\"test-id\""));
        assert!(json.contains("\"method\":\"createChat\""));
        assert!(json.contains("\"params\""));

        let deserialized: Request = serde_json::from_str(&json).unwrap();
        assert_eq!(request, deserialized);
    }

    #[test]
    fn test_response_serialization() {
        let response = Response::success("test-id", serde_json::json!({ "chatId": 123 }));

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"type\":\"response\""));
        assert!(json.contains("\"id\":\"test-id\""));
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"data\""));

        let deserialized: Response = serde_json::from_str(&json).unwrap();
        assert_eq!(response, deserialized);
    }

    #[test]
    fn test_event_serialization() {
        let event = Event::new(
            "chatCreated",
            serde_json::json!({
                "chat": {
                    "id": 123,
                    "title": "New Chat"
                }
            }),
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"event\""));
        assert!(json.contains("\"event\":\"chatCreated\""));
        assert!(json.contains("\"data\""));

        let deserialized: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }
}
