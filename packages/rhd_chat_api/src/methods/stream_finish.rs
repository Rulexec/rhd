//! Stream finish method types.

use serde::{Deserialize, Serialize};

/// Parameters for the `streamFinish` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamFinishParams {
    /// ID of the chat whose stream should be finished.
    pub chat_id: i64,
}

/// Result of the `streamFinish` method.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamFinishResult {
    /// Whether the finish was successful.
    pub success: bool,
}
