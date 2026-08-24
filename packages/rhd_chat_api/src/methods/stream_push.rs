//! Stream push method types.

use serde::{Deserialize, Serialize};

use crate::common::StreamToolCall;

/// Parameters for the `streamPush` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamPushParams {
    /// ID of the chat being streamed.
    pub chat_id: i64,
    /// Reasoning/thinking content delta.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Main content delta.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Tool call deltas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<StreamToolCall>>,
}

/// Result of the `streamPush` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamPushResult {
    /// Whether the push was successful.
    pub success: bool,
}
