//! Error types for the chat WebSocket protocol.
//!
//! This module defines error codes and error response structures
//! used when a request fails.

use serde::{Deserialize, Serialize};

/// Error codes returned by the server.
///
/// # Example JSON
/// ```json
/// "CHAT_NOT_FOUND"
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    /// Chat does not exist.
    #[serde(rename = "CHAT_NOT_FOUND")]
    ChatNotFound,
    /// Message does not exist.
    #[serde(rename = "MESSAGE_NOT_FOUND")]
    MessageNotFound,
    /// Malformed request or missing required fields.
    #[serde(rename = "INVALID_REQUEST")]
    InvalidRequest,
    /// Server-side error.
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCode::ChatNotFound => write!(f, "CHAT_NOT_FOUND"),
            ErrorCode::MessageNotFound => write!(f, "MESSAGE_NOT_FOUND"),
            ErrorCode::InvalidRequest => write!(f, "INVALID_REQUEST"),
            ErrorCode::InternalError => write!(f, "INTERNAL_ERROR"),
        }
    }
}

impl std::error::Error for ErrorCode {}

/// Error response sent when a request fails.
///
/// # Example JSON
/// ```json
/// {
///   "type": "response",
///   "id": "uuid-string",
///   "success": false,
///   "errorCode": "CHAT_NOT_FOUND",
///   "error": "Chat with ID 123 not found"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    /// Always "response".
    pub r#type: String,
    /// Request ID this response corresponds to.
    pub id: String,
    /// Always false for error responses.
    pub success: bool,
    /// Machine-readable error code.
    pub error_code: ErrorCode,
    /// Human-readable error message.
    pub error: String,
}

impl ErrorResponse {
    /// Create a new error response.
    pub fn new(id: impl Into<String>, error_code: ErrorCode, error: impl Into<String>) -> Self {
        ErrorResponse {
            r#type: "response".to_string(),
            id: id.into(),
            success: false,
            error_code,
            error: error.into(),
        }
    }

    /// Create a "chat not found" error response.
    pub fn chat_not_found(id: impl Into<String>, chat_id: i64) -> Self {
        Self::new(
            id,
            ErrorCode::ChatNotFound,
            format!("Chat with ID {} not found", chat_id),
        )
    }

    /// Create a "message not found" error response.
    pub fn message_not_found(id: impl Into<String>, message_id: i64) -> Self {
        Self::new(
            id,
            ErrorCode::MessageNotFound,
            format!("Message with ID {} not found", message_id),
        )
    }

    /// Create an "invalid request" error response.
    pub fn invalid_request(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(id, ErrorCode::InvalidRequest, message)
    }

    /// Create an "internal error" response.
    pub fn internal_error(id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(id, ErrorCode::InternalError, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_serialization() {
        let code = ErrorCode::ChatNotFound;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"CHAT_NOT_FOUND\"");

        let deserialized: ErrorCode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ErrorCode::ChatNotFound);
    }

    #[test]
    fn test_error_code_display() {
        assert_eq!(ErrorCode::ChatNotFound.to_string(), "CHAT_NOT_FOUND");
        assert_eq!(ErrorCode::MessageNotFound.to_string(), "MESSAGE_NOT_FOUND");
        assert_eq!(ErrorCode::InvalidRequest.to_string(), "INVALID_REQUEST");
        assert_eq!(ErrorCode::InternalError.to_string(), "INTERNAL_ERROR");
    }

    #[test]
    fn test_error_response_serialization() {
        let response = ErrorResponse::chat_not_found("test-id", 123);
        let json = serde_json::to_string(&response).unwrap();

        assert!(json.contains("\"type\":\"response\""));
        assert!(json.contains("\"id\":\"test-id\""));
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"errorCode\":\"CHAT_NOT_FOUND\""));
        assert!(json.contains("\"error\":\"Chat with ID 123 not found\""));

        let deserialized: ErrorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, deserialized);
    }

    #[test]
    fn test_error_response_constructors() {
        let not_found = ErrorResponse::chat_not_found("id1", 100);
        assert_eq!(not_found.error_code, ErrorCode::ChatNotFound);
        assert_eq!(not_found.error, "Chat with ID 100 not found");

        let msg_not_found = ErrorResponse::message_not_found("id2", 200);
        assert_eq!(msg_not_found.error_code, ErrorCode::MessageNotFound);
        assert_eq!(msg_not_found.error, "Message with ID 200 not found");

        let invalid = ErrorResponse::invalid_request("id3", "Missing field");
        assert_eq!(invalid.error_code, ErrorCode::InvalidRequest);
        assert_eq!(invalid.error, "Missing field");

        let internal = ErrorResponse::internal_error("id4", "Something went wrong");
        assert_eq!(internal.error_code, ErrorCode::InternalError);
        assert_eq!(internal.error, "Something went wrong");
    }
}
