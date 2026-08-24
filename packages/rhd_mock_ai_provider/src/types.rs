use std::time::Duration;

use rhd_ai_client::{
    ChatCompletionResponse, Choice, FunctionCall,
    ResponseMessage, StreamChunk, ToolCall, Usage,
};
use tokio::sync::mpsc;

pub use rhd_ai_client;

/// Response from the listener
pub enum MockAiResponse {
    /// Non-streaming completion response
    Completion(ChatCompletionResponse),
    /// Streaming response (receiver for chunks)
    Stream(mpsc::Receiver<StreamChunk>),
    /// Error response
    Error { status: u16, message: String },
}

impl MockAiResponse {
    /// Create a text completion response
    pub fn text(content: impl Into<String>) -> Self {
        MockAiResponse::Completion(ChatCompletionResponse {
            id: "mock_resp".to_string(),
            choices: vec![Choice {
                message: ResponseMessage {
                    role: "assistant".to_string(),
                    content: Some(content.into()),
                    tool_calls: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
        })
    }

    /// Create a tool call response
    pub fn tool_call(name: impl Into<String>, arguments: impl Into<String>) -> Self {
        MockAiResponse::Completion(ChatCompletionResponse {
            id: "mock_resp".to_string(),
            choices: vec![Choice {
                message: ResponseMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(vec![ToolCall {
                        id: "call_1".to_string(),
                        call_type: "function".to_string(),
                        function: FunctionCall {
                            name: name.into(),
                            arguments: arguments.into(),
                        },
                    }]),
                },
                finish_reason: Some("tool_calls".to_string()),
            }],
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
        })
    }
}

/// Controller for sending stream chunks from test code
pub struct StreamController {
    sender: mpsc::Sender<StreamChunk>,
}

impl StreamController {
    /// Create a new stream controller and its associated receiver
    pub fn new() -> (Self, mpsc::Receiver<StreamChunk>) {
        let (tx, rx) = mpsc::channel(100);
        (Self { sender: tx }, rx)
    }

    /// Send a chunk immediately
    pub async fn send_chunk(&self, chunk: StreamChunk) -> Result<(), StreamError> {
        self.sender.send(chunk).await.map_err(|_| StreamError::Closed)
    }

    /// Send a text chunk immediately
    pub async fn send_text(&self, content: impl Into<String>) -> Result<(), StreamError> {
        self.send_chunk(StreamChunk {
            reasoning_content: None,
            content: Some(content.into()),
            tool_calls: None,
            finish_reason: None,
        })
        .await
    }

    /// Send a chunk after a delay
    pub async fn send_chunk_delayed(
        &self,
        chunk: StreamChunk,
        delay: Duration,
    ) -> Result<(), StreamError> {
        tokio::time::sleep(delay).await;
        self.send_chunk(chunk).await
    }

    /// Send a text chunk after a delay
    pub async fn send_text_delayed(
        &self,
        content: impl Into<String>,
        delay: Duration,
    ) -> Result<(), StreamError> {
        tokio::time::sleep(delay).await;
        self.send_text(content).await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("stream channel closed")]
    Closed,
}

/// Builder for creating predefined stream responses
pub struct StreamBuilder {
    chunks: Vec<(StreamChunk, Option<Duration>)>,
}

impl StreamBuilder {
    pub fn new() -> Self {
        Self { chunks: Vec::new() }
    }

    /// Add a chunk to be sent immediately
    pub fn chunk(mut self, chunk: StreamChunk) -> Self {
        self.chunks.push((chunk, None));
        self
    }

    /// Add a text chunk to be sent immediately
    pub fn text(self, content: impl Into<String>) -> Self {
        self.chunk(StreamChunk {
            reasoning_content: None,
            content: Some(content.into()),
            tool_calls: None,
            finish_reason: None,
        })
    }

    /// Add a chunk with a delay before sending
    pub fn chunk_delayed(mut self, chunk: StreamChunk, delay: Duration) -> Self {
        self.chunks.push((chunk, Some(delay)));
        self
    }

    /// Add a text chunk with a delay before sending
    pub fn text_delayed(self, content: impl Into<String>, delay: Duration) -> Self {
        self.chunk_delayed(
            StreamChunk {
                reasoning_content: None,
                content: Some(content.into()),
                tool_calls: None,
                finish_reason: None,
            },
            delay,
        )
    }

    /// Add a finish reason to the last chunk
    pub fn finish(mut self, reason: impl Into<String>) -> Self {
        if let Some((chunk, _)) = self.chunks.last_mut() {
            chunk.finish_reason = Some(reason.into());
        } else {
            // If no chunks exist, create an empty one with finish_reason
            self.chunks.push((
                StreamChunk {
                    reasoning_content: None,
                    content: None,
                    tool_calls: None,
                    finish_reason: Some(reason.into()),
                },
                None,
            ));
        }
        self
    }

    /// Build the stream and return the receiver
    /// Spawns a task that sends chunks with delays
    pub fn build(self) -> mpsc::Receiver<StreamChunk> {
        let (controller, receiver) = StreamController::new();

        tokio::spawn(async move {
            for (chunk, delay) in self.chunks {
                if let Some(delay) = delay {
                    tokio::time::sleep(delay).await;
                }
                if controller.send_chunk(chunk).await.is_err() {
                    break;
                }
            }
        });

        receiver
    }
}

impl Default for StreamBuilder {
    fn default() -> Self {
        Self::new()
    }
}
