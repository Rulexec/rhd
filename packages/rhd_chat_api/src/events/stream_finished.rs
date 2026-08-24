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
