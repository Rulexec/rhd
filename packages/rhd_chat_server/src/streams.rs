//! Stream management for real-time chat streaming.
//!
//! This module provides the `StreamManager` which holds active stream state per chat,
//! supports push/subscribe/finish operations, and broadcasts stream chunks to subscribers.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};

// Re-export StreamToolCallDelta from API for convenience
pub use rhd_chat_api::methods::stream_push::StreamToolCallDelta;

/// A chunk of streaming content sent to subscribers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum StreamChunk {
    /// New reasoning/thinking content delta.
    ReasoningDelta { content: String },
    /// New main content delta.
    ContentDelta { content: String },
    /// Tool call delta (accumulated during streaming).
    ToolCallDelta { tool_calls: Vec<StreamToolCallDelta> },
    /// Stream has finished. No more chunks will be sent.
    Finished,
}

/// A snapshot of the current stream state, returned by subscribe_and_get.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSnapshot {
    pub chat_id: i64,
    pub reasoning_content: String,
    pub content: String,
    pub tool_calls: Vec<StreamToolCallDelta>,
    pub is_finished: bool,
}

/// Internal state of an active stream.
struct StreamState {
    reasoning_content: String,
    content: String,
    tool_calls: Vec<StreamToolCallDelta>,
    subscribers: Vec<mpsc::UnboundedSender<StreamChunk>>,
    is_finished: bool,
}

/// Manages active streams for chats.
///
/// Streams are keyed by chat ID (1:1 relationship — only one active stream per chat).
/// The manager supports pushing content deltas, subscribing to stream updates,
/// and finishing streams.
pub struct StreamManager {
    streams: RwLock<HashMap<i64, StreamState>>,
}

/// Shared reference to the StreamManager.
pub type SharedStreamManager = Arc<StreamManager>;

impl StreamManager {
    /// Create a new StreamManager.
    pub fn new() -> Self {
        Self {
            streams: RwLock::new(HashMap::new()),
        }
    }

    /// Push deltas to a stream. Creates the stream if it doesn't exist.
    /// Notifies all subscribers with the new chunk.
    pub async fn push(
        &self,
        chat_id: i64,
        reasoning_delta: Option<String>,
        content_delta: Option<String>,
        tool_calls_delta: Option<Vec<StreamToolCallDelta>>,
    ) {
        let mut streams = self.streams.write().await;
        let state = streams.entry(chat_id).or_insert_with(|| StreamState {
            reasoning_content: String::new(),
            content: String::new(),
            tool_calls: Vec::new(),
            subscribers: Vec::new(),
            is_finished: false,
        });

        if state.is_finished {
            return; // Ignore pushes to finished streams
        }

        // Accumulate content
        if let Some(delta) = &reasoning_delta {
            state.reasoning_content.push_str(delta);
        }
        if let Some(delta) = &content_delta {
            state.content.push_str(delta);
        }
        if let Some(delta) = &tool_calls_delta {
            // Merge tool calls by index, or add new ones
            for new_call in delta {
                if let Some(existing) = state.tool_calls.iter_mut().find(|c| c.index == new_call.index) {
                    existing.arguments.push_str(&new_call.arguments);
                    // Update id and name if they were empty (first chunk had them)
                    if existing.id.is_empty() && !new_call.id.is_empty() {
                        existing.id = new_call.id.clone();
                    }
                    if existing.name.is_empty() && !new_call.name.is_empty() {
                        existing.name = new_call.name.clone();
                    }
                } else {
                    state.tool_calls.push(new_call.clone());
                }
            }
        }

        // Build chunk to send to subscribers
        let chunk = if let Some(delta) = reasoning_delta {
            Some(StreamChunk::ReasoningDelta { content: delta })
        } else if let Some(delta) = content_delta {
            Some(StreamChunk::ContentDelta { content: delta })
        } else if let Some(delta) = tool_calls_delta {
            Some(StreamChunk::ToolCallDelta { tool_calls: delta })
        } else {
            None
        };

        // Notify subscribers
        if let Some(chunk) = chunk {
            state.subscribers.retain(|sender| sender.send(chunk.clone()).is_ok());
        }
    }

    /// Atomically get current stream state AND subscribe to future chunks.
    /// Returns the snapshot and a receiver for future chunks.
    /// If no stream exists for this chat, returns a finished snapshot with empty content.
    pub async fn subscribe_and_get(
        &self,
        chat_id: i64,
    ) -> (StreamSnapshot, mpsc::UnboundedReceiver<StreamChunk>) {
        let mut streams = self.streams.write().await;
        let (sender, receiver) = mpsc::unbounded_channel();

        let state = streams.entry(chat_id).or_insert_with(|| StreamState {
            reasoning_content: String::new(),
            content: String::new(),
            tool_calls: Vec::new(),
            subscribers: Vec::new(),
            is_finished: false,
        });

        let snapshot = StreamSnapshot {
            chat_id,
            reasoning_content: state.reasoning_content.clone(),
            content: state.content.clone(),
            tool_calls: state.tool_calls.clone(),
            is_finished: state.is_finished,
        };

        // Only subscribe if stream is not finished
        if !state.is_finished {
            state.subscribers.push(sender);
        } else {
            // Stream already finished, send Finished chunk immediately
            let _ = sender.send(StreamChunk::Finished);
            // Drop sender so receiver gets the Finished message then closes
        }

        (snapshot, receiver)
    }

    /// Finish a stream. Returns the final snapshot.
    /// Sends Finished chunk to all subscribers and clears them.
    /// The stream entry is removed after finishing.
    pub async fn finish(&self, chat_id: i64) -> StreamSnapshot {
        let mut streams = self.streams.write().await;

        let state = match streams.remove(&chat_id) {
            Some(state) => state,
            None => {
                // No stream exists — return empty finished snapshot
                return StreamSnapshot {
                    chat_id,
                    reasoning_content: String::new(),
                    content: String::new(),
                    tool_calls: Vec::new(),
                    is_finished: true,
                };
            }
        };

        let snapshot = StreamSnapshot {
            chat_id,
            reasoning_content: state.reasoning_content.clone(),
            content: state.content.clone(),
            tool_calls: state.tool_calls.clone(),
            is_finished: true,
        };

        // Notify all subscribers that stream is finished
        for sender in &state.subscribers {
            let _ = sender.send(StreamChunk::Finished);
        }
        // Dropping state drops all senders, closing the channels

        snapshot
    }

    /// Check if a stream exists and is active (not finished) for a chat.
    pub async fn is_active(&self, chat_id: i64) -> bool {
        let streams = self.streams.read().await;
        streams.get(&chat_id).map_or(false, |s| !s.is_finished)
    }
}

impl Default for StreamManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stream_push_and_subscribe() {
        let manager = StreamManager::new();
        let chat_id = 1;

        // Push reasoning delta
        manager
            .push(chat_id, Some("Thinking...".to_string()), None, None)
            .await;

        // Push content delta
        manager
            .push(chat_id, None, Some("Hello".to_string()), None)
            .await;

        // Subscribe and get snapshot
        let (snapshot, mut receiver) = manager.subscribe_and_get(chat_id).await;

        assert_eq!(snapshot.chat_id, chat_id);
        assert_eq!(snapshot.reasoning_content, "Thinking...");
        assert_eq!(snapshot.content, "Hello");
        assert!(!snapshot.is_finished);

        // Push more content
        manager
            .push(chat_id, None, Some(" World".to_string()), None)
            .await;

        // Receiver should get the new chunk
        let chunk = receiver.recv().await.unwrap();
        match chunk {
            StreamChunk::ContentDelta { content } => {
                assert_eq!(content, " World");
            }
            _ => panic!("Expected ContentDelta chunk"),
        }
    }

    #[tokio::test]
    async fn test_stream_finish() {
        let manager = StreamManager::new();
        let chat_id = 1;

        // Create stream and push content
        manager
            .push(chat_id, None, Some("Hello".to_string()), None)
            .await;

        // Subscribe to get receiver
        let (_snapshot, mut receiver) = manager.subscribe_and_get(chat_id).await;

        // Finish the stream
        let final_snapshot = manager.finish(chat_id).await;

        assert_eq!(final_snapshot.content, "Hello");
        assert!(final_snapshot.is_finished);

        // Receiver should get Finished chunk
        let chunk = receiver.recv().await.unwrap();
        matches!(chunk, StreamChunk::Finished);

        // After finish, stream is removed. Subsequent push creates a new stream.
        manager
            .push(chat_id, None, Some(" More".to_string()), None)
            .await;

        // New subscribe should get the new stream's content
        let (snapshot, _receiver) = manager.subscribe_and_get(chat_id).await;
        assert_eq!(snapshot.content, " More");
        assert!(!snapshot.is_finished); // New stream is not finished
    }

    #[tokio::test]
    async fn test_stream_subscribe_after_finish() {
        let manager = StreamManager::new();
        let chat_id = 1;

        // Create and finish stream
        manager
            .push(chat_id, None, Some("Hello".to_string()), None)
            .await;
        manager.finish(chat_id).await;

        // Subscribe after finish - creates a new stream (not finished)
        let (snapshot, _receiver) = manager.subscribe_and_get(chat_id).await;

        assert_eq!(snapshot.content, "");
        assert!(!snapshot.is_finished); // New stream is not finished
    }

    #[tokio::test]
    async fn test_stream_multiple_subscribers() {
        let manager = StreamManager::new();
        let chat_id = 1;

        // Create two subscribers
        let (_snapshot1, mut receiver1) = manager.subscribe_and_get(chat_id).await;
        let (_snapshot2, mut receiver2) = manager.subscribe_and_get(chat_id).await;

        // Push content
        manager
            .push(chat_id, None, Some("Hello".to_string()), None)
            .await;

        // Both receivers should get the chunk
        let chunk1 = receiver1.recv().await.unwrap();
        let chunk2 = receiver2.recv().await.unwrap();

        match chunk1 {
            StreamChunk::ContentDelta { content } => {
                assert_eq!(content, "Hello");
            }
            _ => panic!("Expected ContentDelta chunk"),
        }

        match chunk2 {
            StreamChunk::ContentDelta { content } => {
                assert_eq!(content, "Hello");
            }
            _ => panic!("Expected ContentDelta chunk"),
        }
    }

    #[tokio::test]
    async fn test_stream_nonexistent_chat() {
        let manager = StreamManager::new();
        let chat_id = 999;

        // Subscribe to non-existent chat
        let (snapshot, mut receiver) = manager.subscribe_and_get(chat_id).await;

        assert_eq!(snapshot.chat_id, chat_id);
        assert_eq!(snapshot.content, "");
        assert!(!snapshot.is_finished); // Stream created but not finished

        // Finish non-existent chat (should not panic)
        let finish_snapshot = manager.finish(chat_id).await;
        assert!(finish_snapshot.is_finished);
        assert_eq!(finish_snapshot.content, "");

        // Receiver should get Finished
        let chunk = receiver.recv().await.unwrap();
        matches!(chunk, StreamChunk::Finished);
    }

    #[tokio::test]
    async fn test_stream_tool_calls_merge() {
        let manager = StreamManager::new();
        let chat_id = 1;

        // Push initial tool call
        let tool_call1 = StreamToolCallDelta {
            index: 0,
            id: "tool1".to_string(),
            name: "search".to_string(),
            arguments: "{\"query\":".to_string(),
        };
        manager
            .push(chat_id, None, None, Some(vec![tool_call1]))
            .await;

        // Push more arguments to same tool call (same index)
        let tool_call2 = StreamToolCallDelta {
            index: 0,
            id: "tool1".to_string(),
            name: "search".to_string(),
            arguments: "\"test\"}".to_string(),
        };
        manager
            .push(chat_id, None, None, Some(vec![tool_call2]))
            .await;

        // Subscribe and check merged arguments
        let (snapshot, _receiver) = manager.subscribe_and_get(chat_id).await;

        assert_eq!(snapshot.tool_calls.len(), 1);
        assert_eq!(snapshot.tool_calls[0].id, "tool1");
        assert_eq!(
            snapshot.tool_calls[0].arguments,
            "{\"query\":\"test\"}"
        );
    }

    #[tokio::test]
    async fn test_stream_is_active() {
        let manager = StreamManager::new();
        let chat_id = 1;

        // Initially not active
        assert!(!manager.is_active(chat_id).await);

        // Push to create stream
        manager
            .push(chat_id, None, Some("Hello".to_string()), None)
            .await;

        // Now active
        assert!(manager.is_active(chat_id).await);

        // Finish stream
        manager.finish(chat_id).await;

        // No longer active
        assert!(!manager.is_active(chat_id).await);
    }
}
