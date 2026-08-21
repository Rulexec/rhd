//! Server error types.

use thiserror::Error;

/// Server error types.
#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Database error: {0}")]
    Database(#[from] rhd_db::DbError),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ServerError {
    /// Convert to error code for API response.
    pub fn to_error_code(&self) -> rhd_chat_api::ErrorCode {
        match self {
            ServerError::Database(_) => rhd_chat_api::ErrorCode::InternalError,
            ServerError::WebSocket(_) => rhd_chat_api::ErrorCode::InternalError,
            ServerError::Json(_) => rhd_chat_api::ErrorCode::InvalidRequest,
            ServerError::Io(_) => rhd_chat_api::ErrorCode::InternalError,
            ServerError::Internal(_) => rhd_chat_api::ErrorCode::InternalError,
        }
    }
}
