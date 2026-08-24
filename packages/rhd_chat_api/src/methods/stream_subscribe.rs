//! Stream subscribe method types.

use serde::{Deserialize, Serialize};

use crate::common::StreamToolCall;

/// Parameters for the `streamSubscribe` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSubscribeParams {
    /// ID of the chat to subscribe to.
    pub chat_id: i64,
}

/// Result of the `streamSubscribe` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSubscribeResult {
    /// Current reasoning content accumulated so far.
    pub reasoning_content: String,
    /// Current main content accumulated so far.
    pub content: String,
    /// Current tool calls accumulated so far.
    pub tool_calls: Vec<StreamToolCall>,
    /// Whether the stream has already finished.
    pub is_finished: bool,
}
