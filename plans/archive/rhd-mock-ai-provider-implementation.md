# RHD Mock AI Provider Implementation Plan

## Overview

This plan describes the implementation of two new packages:
1. **rhd_ai_client** - A client library for OpenAI-compatible chat completions API
2. **rhd_mock_ai_provider** - A mock AI provider server with listener-based response control

These packages will replace the existing mock implementation in `rhd_test` and provide a reusable, testable AI provider interface.

## Package 1: rhd_ai_client

### Purpose
A lightweight client library for communicating with OpenAI-compatible chat completions endpoints. This will be used by `rhd_mock_ai_provider` in tests and can be used by other packages that need to interact with AI providers.

### Structure
```
packages/rhd_ai_client/
├── Cargo.toml
└── src/
    ├── lib.rs              # Public API exports
    ├── client.rs           # Main client implementation
    ├── types.rs            # Request/response types
    ├── error.rs            # Error types
    └── stream.rs           # Streaming response handling
```

### API Design

#### Client Interface
```rust
pub struct AiClient {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl AiClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self;
    
    /// Non-streaming chat completion
    pub async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, AiClientError>;
    
    /// Streaming chat completion
    pub async fn chat_completion_stream(
        &self,
        request: ChatCompletionRequest,
        on_chunk: impl FnMut(StreamChunk) -> bool,
    ) -> Result<StreamResult, AiClientError>;
}
```

#### Types
```rust
// Request types
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub stream: bool,
}

pub enum ChatMessage {
    System { content: String },
    User { content: String },
    Assistant { 
        content: Option<String>,
        tool_calls: Option<Vec<ToolCall>>,
    },
    Tool { 
        tool_call_id: String,
        content: String,
    },
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

// Response types
pub struct ChatCompletionResponse {
    pub id: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

pub struct Choice {
    pub message: ResponseMessage,
    pub finish_reason: Option<String>,
}

pub struct ResponseMessage {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

pub struct ToolCall {
    pub id: String,
    pub call_type: String,
    pub function: FunctionCall,
}

pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

// Streaming types
pub struct StreamChunk {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallDelta>>,
    pub finish_reason: Option<String>,
}

pub struct StreamResult {
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
}
```

### Dependencies
```toml
[dependencies]
reqwest = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
futures-util = { workspace = true }
```

## Package 2: rhd_mock_ai_provider

### Purpose
A mock AI provider server that:
- Starts on a random available port
- Provides a getter for the port
- Uses a listener pattern to control responses
- Supports both streaming and non-streaming modes
- Supports tool calls and tool definitions
- **Allows dynamic control of streaming with delays between chunks**

### Structure
```
packages/rhd_mock_ai_provider/
├── Cargo.toml
└── src/
    ├── lib.rs              # Public API exports
    ├── server.rs           # Mock server implementation
    ├── listener.rs         # Listener trait and types
    ├── types.rs            # Shared types
    └── handlers.rs         # Request handlers
```

### API Design

#### Listener Trait
```rust
/// Trait for controlling mock AI responses
pub trait MockAiListener: Send + Sync + 'static {
    /// Called when a chat completion request is received
    /// Returns the response to send back
    fn on_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> impl Future<Output = MockAiResponse> + Send;
}

/// Response from the listener
pub enum MockAiResponse {
    /// Non-streaming response
    Completion(ChatCompletionResponse),
    /// Streaming response with controlled chunk emission
    Stream(StreamController),
    /// Tool call response
    ToolCall(ToolCallResponse),
    /// Error response
    Error { status: u16, message: String },
}
```

#### Stream Controller
The `StreamController` allows tests to emit chunks dynamically with delays:

```rust
/// Controller for streaming responses
pub struct StreamController {
    sender: mpsc::Sender<StreamChunk>,
}

impl StreamController {
    /// Create a new stream controller
    pub fn new() -> (Self, mpsc::Receiver<StreamChunk>);
    
    /// Send a chunk immediately
    pub async fn send_chunk(&self, chunk: StreamChunk) -> Result<(), StreamError>;
    
    /// Send a text chunk immediately
    pub async fn send_text(&self, content: impl Into<String>) -> Result<(), StreamError>;
    
    /// Send a chunk after a delay
    pub async fn send_chunk_delayed(
        &self, 
        chunk: StreamChunk, 
        delay: Duration
    ) -> Result<(), StreamError>;
    
    /// Send a text chunk after a delay
    pub async fn send_text_delayed(
        &self, 
        content: impl Into<String>, 
        delay: Duration
    ) -> Result<(), StreamError>;
    
    /// Close the stream (signals completion)
    pub fn close(self);
}

/// Builder for creating stream responses with predefined chunks
pub struct StreamBuilder {
    chunks: Vec<(StreamChunk, Option<Duration>)>,
}

impl StreamBuilder {
    pub fn new() -> Self;
    
    /// Add a chunk to be sent immediately
    pub fn chunk(mut self, chunk: StreamChunk) -> Self;
    
    /// Add a text chunk to be sent immediately
    pub fn text(mut self, content: impl Into<String>) -> Self;
    
    /// Add a chunk with a delay before sending
    pub fn chunk_delayed(mut self, chunk: StreamChunk, delay: Duration) -> Self;
    
    /// Add a text chunk with a delay before sending
    pub fn text_delayed(mut self, content: impl Into<String>, delay: Duration) -> Self;
    
    /// Build the stream response
    pub fn build(self) -> MockAiResponse;
}
```

#### Server Interface
```rust
pub struct MockAiProvider {
    port: u16,
    listener: Arc<dyn MockAiListener>,
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl MockAiProvider {
    /// Start the mock server with the given listener
    pub async fn start<L: MockAiListener>(listener: L) -> Result<Self, MockAiError>;
    
    /// Get the port the server is running on
    pub fn port(&self) -> u16;
    
    /// Get the base URL for the API
    pub fn base_url(&self) -> String;
    
    /// Shutdown the server
    pub async fn shutdown(self);
}
```

#### Built-in Listeners
```rust
/// Simple listener that returns predefined responses
pub struct SimpleListener {
    responses: Arc<Mutex<VecDeque<MockAiResponse>>>,
}

impl SimpleListener {
    pub fn new() -> Self;
    
    /// Add a response to the queue
    pub fn push_response(&self, response: MockAiResponse);
    
    /// Add a text completion response
    pub fn push_text(&self, content: impl Into<String>);
    
    /// Add a tool call response
    pub fn push_tool_call(&self, name: impl Into<String>, arguments: impl Into<String>);
    
    /// Add a streaming response using StreamBuilder
    pub fn push_stream(&self, builder: StreamBuilder);
}

/// Recording listener that records all requests and returns predefined responses
pub struct RecordingListener {
    simple: SimpleListener,
    requests: Arc<Mutex<Vec<ChatCompletionRequest>>>,
}

impl RecordingListener {
    pub fn new() -> Self;
    
    /// Get all recorded requests
    pub fn get_requests(&self) -> Vec<ChatCompletionRequest>;
    
    /// Get the last request
    pub fn last_request(&self) -> Option<ChatCompletionRequest>;
    
    // Inherit all methods from SimpleListener
    pub fn push_response(&self, response: MockAiResponse);
    pub fn push_text(&self, content: impl Into<String>);
    pub fn push_tool_call(&self, name: impl Into<String>, arguments: impl Into<String>);
    pub fn push_stream(&self, builder: StreamBuilder);
}

/// Dynamic listener that allows runtime control of responses
pub struct DynamicListener {
    response_tx: mpsc::Sender<MockAiResponse>,
    requests: Arc<Mutex<Vec<ChatCompletionRequest>>>,
}

impl DynamicListener {
    pub fn new() -> (Self, mpsc::Receiver<MockAiResponse>);
    
    /// Queue a response to be returned for the next request
    pub async fn queue_response(&self, response: MockAiResponse);
    
    /// Get all recorded requests
    pub fn get_requests(&self) -> Vec<ChatCompletionRequest>;
}
```

### Dependencies
```toml
[dependencies]
axum = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
futures-util = { workspace = true }
tokio-stream = { workspace = true }
rhd_ai_client = { path = "../rhd_ai_client" }

[dev-dependencies]
tokio = { workspace = true, features = ["full", "test-util"] }
```

## Integration Tests

### Test Location
```
packages/rhd_mock_ai_provider/tests/
└── integration_tests.rs
```

### Test Scenarios

#### 1. Basic Server Startup
```rust
#[tokio::test]
async fn test_server_starts_on_random_port() {
    let listener = SimpleListener::new();
    let provider = MockAiProvider::start(listener).await.unwrap();
    
    assert!(provider.port() > 0);
    assert!(provider.base_url().starts_with("http://127.0.0.1:"));
    
    provider.shutdown().await;
}
```

#### 2. Non-Streaming Text Completion
```rust
#[tokio::test]
async fn test_non_streaming_text_completion() {
    let listener = RecordingListener::new();
    listener.push_text("Hello, world!");
    
    let provider = MockAiProvider::start(listener.clone()).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");
    
    let request = ChatCompletionRequest {
        model: "test-model".to_string(),
        messages: vec![ChatMessage::User { content: "Hi".to_string() }],
        tools: None,
        stream: false,
    };
    
    let response = client.chat_completion(request).await.unwrap();
    assert_eq!(response.choices[0].message.content.as_deref(), Some("Hello, world!"));
    
    // Verify request was recorded
    let requests = listener.get_requests();
    assert_eq!(requests.len(), 1);
    
    provider.shutdown().await;
}
```

#### 3. Streaming with Predefined Chunks (StreamBuilder)
```rust
#[tokio::test]
async fn test_streaming_with_predefined_chunks() {
    let listener = SimpleListener::new();
    
    // Use StreamBuilder to create a stream with delays
    let stream = StreamBuilder::new()
        .text("Hello")
        .text_delayed(", ", Duration::from_millis(100))
        .text_delayed("world!", Duration::from_millis(100))
        .build();
    
    listener.push_stream(stream);
    
    let provider = MockAiProvider::start(listener).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");
    
    let request = ChatCompletionRequest {
        model: "test-model".to_string(),
        messages: vec![ChatMessage::User { content: "Hi".to_string() }],
        tools: None,
        stream: true,
    };
    
    let mut chunks = Vec::new();
    let start = std::time::Instant::now();
    let result = client.chat_completion_stream(request, |chunk| {
        chunks.push(chunk);
        true
    }).await.unwrap();
    
    let elapsed = start.elapsed();
    
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].content.as_deref(), Some("Hello"));
    assert_eq!(chunks[1].content.as_deref(), Some(", "));
    assert_eq!(chunks[2].content.as_deref(), Some("world!"));
    
    // Verify delays were applied (should take at least 200ms)
    assert!(elapsed >= Duration::from_millis(200));
    
    provider.shutdown().await;
}
```

#### 4. Dynamic Stream Control with StreamController
```rust
#[tokio::test]
async fn test_dynamic_stream_control() {
    let listener = DynamicListener::new();
    let (response_tx, mut response_rx) = mpsc::channel(10);
    
    let provider = MockAiProvider::start(listener.0).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");
    
    // Spawn task to handle the stream
    let stream_task = tokio::spawn(async move {
        let request = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![ChatMessage::User { content: "Hi".to_string() }],
            tools: None,
            stream: true,
        };
        
        let mut chunks = Vec::new();
        client.chat_completion_stream(request, |chunk| {
            chunks.push(chunk);
            true
        }).await.unwrap();
        
        chunks
    });
    
    // Wait for request to arrive
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    // Create stream controller
    let (controller, receiver) = StreamController::new();
    
    // Queue the stream response
    listener.0.queue_response(MockAiResponse::Stream(controller)).await;
    
    // Send chunks with delays
    controller.send_text("Hello").await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    controller.send_text(", ").await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    controller.send_text("world!").await.unwrap();
    controller.close();
    
    let chunks = stream_task.await.unwrap();
    
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].content.as_deref(), Some("Hello"));
    assert_eq!(chunks[1].content.as_deref(), Some(", "));
    assert_eq!(chunks[2].content.as_deref(), Some("world!"));
    
    provider.shutdown().await;
}
```

#### 5. Tool Call Response
```rust
#[tokio::test]
async fn test_tool_call_response() {
    let listener = RecordingListener::new();
    listener.push_tool_call("get_weather", r#"{"city":"London"}"#);
    
    let provider = MockAiProvider::start(listener.clone()).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");
    
    let request = ChatCompletionRequest {
        model: "test-model".to_string(),
        messages: vec![ChatMessage::User { content: "What's the weather?".to_string() }],
        tools: Some(vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather for a city".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                }
            }),
        }]),
        stream: false,
    };
    
    let response = client.chat_completion(request).await.unwrap();
    assert_eq!(response.choices[0].finish_reason.as_deref(), Some("tool_calls"));
    
    let tool_calls = response.choices[0].message.tool_calls.as_ref().unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].function.name, "get_weather");
    assert_eq!(tool_calls[0].function.arguments, r#"{"city":"London"}"#);
    
    provider.shutdown().await;
}
```

#### 6. Tool Call with Tool Result
```rust
#[tokio::test]
async fn test_tool_call_with_result() {
    let listener = RecordingListener::new();
    
    // First response: tool call
    listener.push_tool_call("get_weather", r#"{"city":"London"}"#);
    // Second response: text after tool result
    listener.push_text("The weather in London is sunny.");
    
    let provider = MockAiProvider::start(listener.clone()).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");
    
    // First request - should get tool call
    let request1 = ChatCompletionRequest {
        model: "test-model".to_string(),
        messages: vec![ChatMessage::User { content: "What's the weather?".to_string() }],
        tools: Some(vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather".to_string(),
            parameters: serde_json::json!({}),
        }]),
        stream: false,
    };
    
    let response1 = client.chat_completion(request1).await.unwrap();
    let tool_call = &response1.choices[0].message.tool_calls.as_ref().unwrap()[0];
    
    // Second request - with tool result
    let request2 = ChatCompletionRequest {
        model: "test-model".to_string(),
        messages: vec![
            ChatMessage::User { content: "What's the weather?".to_string() },
            ChatMessage::Assistant { 
                content: None,
                tool_calls: Some(vec![tool_call.clone()]),
            },
            ChatMessage::Tool { 
                tool_call_id: tool_call.id.clone(),
                content: r#"{"temperature":"20C","condition":"sunny"}"#.to_string(),
            },
        ],
        tools: None,
        stream: false,
    };
    
    let response2 = client.chat_completion(request2).await.unwrap();
    assert_eq!(
        response2.choices[0].message.content.as_deref(),
        Some("The weather in London is sunny.")
    );
    
    // Verify both requests were recorded
    let requests = listener.get_requests();
    assert_eq!(requests.len(), 2);
    
    provider.shutdown().await;
}
```

#### 7. Streaming Tool Calls
```rust
#[tokio::test]
async fn test_streaming_tool_calls() {
    let listener = SimpleListener::new();
    
    // Create a stream with tool call deltas
    let stream = StreamBuilder::new()
        .chunk(StreamChunk {
            content: None,
            tool_calls: Some(vec![ToolCallDelta {
                index: 0,
                id: Some("call_1".to_string()),
                call_type: Some("function".to_string()),
                function: Some(FunctionCallDelta {
                    name: Some("get_weather".to_string()),
                    arguments: Some("".to_string()),
                }),
            }],
            finish_reason: None,
        })
        .chunk_delayed(StreamChunk {
            content: None,
            tool_calls: Some(vec![ToolCallDelta {
                index: 0,
                id: None,
                call_type: None,
                function: Some(FunctionCallDelta {
                    name: None,
                    arguments: Some(r#"{"city":"#.to_string()),
                }),
            }],
            finish_reason: None,
        }, Duration::from_millis(50))
        .chunk_delayed(StreamChunk {
            content: None,
            tool_calls: Some(vec![ToolCallDelta {
                index: 0,
                id: None,
                call_type: None,
                function: Some(FunctionCallDelta {
                    name: None,
                    arguments: Some(r#""London"}"#.to_string()),
                }),
            }],
            finish_reason: None,
        }, Duration::from_millis(50))
        .chunk(StreamChunk {
            content: None,
            tool_calls: None,
            finish_reason: Some("tool_calls".to_string()),
        })
        .build();
    
    listener.push_stream(stream);
    
    let provider = MockAiProvider::start(listener).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");
    
    let request = ChatCompletionRequest {
        model: "test-model".to_string(),
        messages: vec![ChatMessage::User { content: "What's the weather?".to_string() }],
        tools: Some(vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: "Get weather".to_string(),
            parameters: serde_json::json!({}),
        }]),
        stream: true,
    };
    
    let mut chunks = Vec::new();
    let result = client.chat_completion_stream(request, |chunk| {
        chunks.push(chunk);
        true
    }).await.unwrap();
    
    assert_eq!(result.finish_reason.as_deref(), Some("tool_calls"));
    
    provider.shutdown().await;
}
```

#### 8. Multiple Sequential Requests
```rust
#[tokio::test]
async fn test_multiple_sequential_requests() {
    let listener = RecordingListener::new();
    listener.push_text("First response");
    listener.push_text("Second response");
    listener.push_text("Third response");
    
    let provider = MockAiProvider::start(listener.clone()).await.unwrap();
    let client = AiClient::new(provider.base_url(), "test-key");
    
    for i in 1..=3 {
        let request = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![ChatMessage::User { content: format!("Message {}", i) }],
            tools: None,
            stream: false,
        };
        
        let response = client.chat_completion(request).await.unwrap();
        assert_eq!(
            response.choices[0].message.content.as_deref(),
            Some(format!("{} response", match i { 1 => "First", 2 => "Second", 3 => "Third", _ => unreachable!() }).as_str())
        );
    }
    
    assert_eq!(listener.get_requests().len(), 3);
    
    provider.shutdown().await;
}
```

## Implementation Order

### Phase 1: rhd_ai_client
1. Create package structure and Cargo.toml
2. Implement types module (request/response types)
3. Implement error module
4. Implement client module with non-streaming support
5. Implement stream module for streaming support
6. Add unit tests for client

### Phase 2: rhd_mock_ai_provider
1. Create package structure and Cargo.toml
2. Implement types module
3. Implement StreamController and StreamBuilder
4. Implement listener trait and SimpleListener
5. Implement RecordingListener
6. Implement DynamicListener
7. Implement server module with Axum
8. Implement handlers for chat completions
9. Add integration tests

### Phase 3: Integration
1. Update workspace Cargo.toml to include new packages
2. Verify all tests pass
3. Update documentation

## Key Design Decisions

### 1. Stream Control
The `StreamController` and `StreamBuilder` provide two ways to control streaming:
- **StreamBuilder**: For predefined sequences of chunks with optional delays
- **StreamController**: For dynamic, runtime control of chunk emission

This allows tests to simulate realistic streaming behavior with delays between chunks.

### 2. Listener Pattern
The listener pattern allows tests to dynamically control responses without restarting the server. Three listener types are provided:
- **SimpleListener**: Queue-based responses
- **RecordingListener**: Records requests and returns queued responses
- **DynamicListener**: Runtime control via channels

### 3. Separate Client Package
Creating a separate `rhd_ai_client` package allows:
- Reuse in tests and other packages
- Clear separation of concerns
- Independent versioning

### 4. Type Compatibility
Types in `rhd_ai_client` will be compatible with but not identical to types in `rhd_ai`. This allows:
- Independent evolution
- Clear API boundaries
- Easier testing

### 5. Streaming Implementation
Streaming will use SSE (Server-Sent Events) format compatible with OpenAI's API. The client will provide a callback-based API for processing chunks.

## Migration Notes

The existing `rhd_test` mock server will remain unchanged during this implementation. Once the new packages are complete and tested, `rhd_test` can be updated to use `rhd_mock_ai_provider` if desired.
