//! Error types for the chat client.

use thiserror::Error;

/// Errors that can occur when using the chat client.
#[derive(Debug, Error)]
pub enum ClientError {
    /// WebSocket connection or communication error.
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    /// The connection was closed unexpectedly.
    #[error("Connection closed")]
    ConnectionClosed,

    /// The request timed out waiting for a response.
    #[error("Request timeout: {0}")]
    Timeout(String),

    /// The server returned an error response.
    #[error("Server error: {code} - {message}")]
    Server {
        /// Error code from the server.
        code: String,
        /// Error message from the server.
        message: String,
    },

    /// JSON serialization or deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Internal channel error (should not happen in normal operation).
    #[error("Internal channel error")]
    ChannelError,
}
