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

    /// Create a streaming text response
    pub fn stream_text(content: impl Into<String>) -> Self {
        let (controller, receiver) = StreamController::new();
        let content_str = content.into();
        
        tokio::spawn(async move {
            // Send content in chunks
            for chunk in content_str.chars().collect::<Vec<_>>().chunks(10) {
                let chunk_str: String = chunk.iter().collect();
                if controller.send_text(chunk_str).await.is_err() {
                    break;
                }
            }
            // Send finish reason
            let _ = controller.send_chunk(StreamChunk {
                reasoning_content: None,
                content: None,
                tool_calls: None,
                finish_reason: Some("stop".to_string()),
            }).await;
        });
        
        MockAiResponse::Stream(receiver)
    }

    /// Create a streaming tool call response
    pub fn stream_tool_call(name: impl Into<String>, arguments: impl Into<String>) -> Self {
        let (controller, receiver) = StreamController::new();
        let name_str = name.into();
        let arguments_str = arguments.into();
        
        tokio::spawn(async move {
            // Send tool call chunk
            let _ = controller.send_chunk(StreamChunk {
                reasoning_content: None,
                content: None,
                tool_calls: Some(vec![rhd_ai_client::ToolCallDelta {
                    index: 0,
                    id: Some("call_1".to_string()),
                    call_type: Some("function".to_string()),
                    function: Some(rhd_ai_client::FunctionCallDelta {
                        name: Some(name_str),
                        arguments: Some(arguments_str),
                    }),
                }]),
                finish_reason: None,
            }).await;
            
            // Send finish reason
            let _ = controller.send_chunk(StreamChunk {
                reasoning_content: None,
                content: None,
                tool_calls: None,
                finish_reason: Some("tool_calls".to_string()),
            }).await;
        });
        
        MockAiResponse::Stream(receiver)
    }

    /// Create a streaming tool call response that simulates real OpenAI behavior.
    ///
    /// Real OpenAI API sends tool calls in multiple chunks:
    /// 1. First chunk: index=0, id="call_123", name="function_name", arguments=""
    /// 2. Subsequent chunks: index=0, id=None, name=None, arguments="{\"partial\": ...}"
    ///
    /// This method splits the arguments into multiple chunks to simulate this behavior.
    pub fn stream_tool_call_chunked(name: impl Into<String>, arguments: impl Into<String>) -> Self {
        let (controller, receiver) = StreamController::new();
        let name_str = name.into();
        let arguments_str = arguments.into();

        tokio::spawn(async move {
            // First chunk: id and name, empty arguments
            let _ = controller.send_chunk(StreamChunk {
                reasoning_content: None,
                content: None,
                tool_calls: Some(vec![rhd_ai_client::ToolCallDelta {
                    index: 0,
                    id: Some("call_chunked_1".to_string()),
                    call_type: Some("function".to_string()),
                    function: Some(rhd_ai_client::FunctionCallDelta {
                        name: Some(name_str),
                        arguments: Some(String::new()),
                    }),
                }]),
                finish_reason: None,
            }).await;

            // Subsequent chunks: no id/name, only arguments fragments
            for chunk in arguments_str.chars().collect::<Vec<_>>().chunks(5) {
                let chunk_str: String = chunk.iter().collect();
                let _ = controller.send_chunk(StreamChunk {
                    reasoning_content: None,
                    content: None,
                    tool_calls: Some(vec![rhd_ai_client::ToolCallDelta {
                        index: 0,
                        id: None,
                        call_type: None,
                        function: Some(rhd_ai_client::FunctionCallDelta {
                            name: None,
                            arguments: Some(chunk_str),
                        }),
                    }]),
                    finish_reason: None,
                }).await;
            }

            // Send finish reason
            let _ = controller.send_chunk(StreamChunk {
                reasoning_content: None,
                content: None,
                tool_calls: None,
                finish_reason: Some("tool_calls".to_string()),
            }).await;
        });

        MockAiResponse::Stream(receiver)
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
