use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use rhd_ai_client::ChatCompletionRequest;
use tokio::sync::mpsc;

use crate::types::MockAiResponse;

/// Trait for controlling mock AI responses
pub trait MockAiListener: Send + Sync + 'static {
    /// Called when a chat completion request is received.
    /// Returns the response to send back.
    fn on_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Pin<Box<dyn Future<Output = MockAiResponse> + Send>>;
}

/// Simple listener that returns predefined responses from a queue
#[derive(Clone)]
pub struct SimpleListener {
    responses: Arc<Mutex<VecDeque<MockAiResponse>>>,
}

impl SimpleListener {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Add a response to the queue
    pub fn push_response(&self, response: MockAiResponse) {
        self.responses.lock().unwrap().push_back(response);
    }

    /// Add a text completion response
    pub fn push_text(&self, content: impl Into<String>) {
        self.push_response(MockAiResponse::text(content));
    }

    /// Add a tool call response
    pub fn push_tool_call(&self, name: impl Into<String>, arguments: impl Into<String>) {
        self.push_response(MockAiResponse::tool_call(name, arguments));
    }

    /// Add an error response
    pub fn push_error(&self, status: u16, message: impl Into<String>) {
        self.push_response(MockAiResponse::Error {
            status,
            message: message.into(),
        });
    }

    /// Add a streaming response from a receiver
    pub fn push_stream_receiver(&self, receiver: mpsc::Receiver<rhd_ai_client::StreamChunk>) {
        self.push_response(MockAiResponse::Stream(receiver));
    }
}

impl Default for SimpleListener {
    fn default() -> Self {
        Self::new()
    }
}

impl MockAiListener for SimpleListener {
    fn on_chat_completion(
        &self,
        _request: ChatCompletionRequest,
    ) -> Pin<Box<dyn Future<Output = MockAiResponse> + Send>> {
        let response = self.responses.lock().unwrap().pop_front().unwrap_or_else(|| {
            MockAiResponse::text("No response configured")
        });
        Box::pin(async move { response })
    }
}

/// Recording listener that records all requests and returns predefined responses
#[derive(Clone)]
pub struct RecordingListener {
    inner: SimpleListener,
    requests: Arc<Mutex<Vec<ChatCompletionRequest>>>,
}

impl RecordingListener {
    pub fn new() -> Self {
        Self {
            inner: SimpleListener::new(),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get all recorded requests
    pub fn get_requests(&self) -> Vec<ChatCompletionRequest> {
        self.requests.lock().unwrap().clone()
    }

    /// Get the last request
    pub fn last_request(&self) -> Option<ChatCompletionRequest> {
        self.requests.lock().unwrap().last().cloned()
    }

    /// Add a response to the queue
    pub fn push_response(&self, response: MockAiResponse) {
        self.inner.push_response(response);
    }

    /// Add a text completion response
    pub fn push_text(&self, content: impl Into<String>) {
        self.inner.push_text(content);
    }

    /// Add a tool call response
    pub fn push_tool_call(&self, name: impl Into<String>, arguments: impl Into<String>) {
        self.inner.push_tool_call(name, arguments);
    }

    /// Add an error response
    pub fn push_error(&self, status: u16, message: impl Into<String>) {
        self.inner.push_error(status, message);
    }

    /// Add a streaming response from a receiver
    pub fn push_stream_receiver(&self, receiver: mpsc::Receiver<rhd_ai_client::StreamChunk>) {
        self.inner.push_stream_receiver(receiver);
    }
}

impl Default for RecordingListener {
    fn default() -> Self {
        Self::new()
    }
}

impl MockAiListener for RecordingListener {
    fn on_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Pin<Box<dyn Future<Output = MockAiResponse> + Send>> {
        self.requests.lock().unwrap().push(request.clone());
        self.inner.on_chat_completion(request)
    }
}

/// Dynamic listener that allows runtime control of responses via channels
pub struct DynamicListener {
    response_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<MockAiResponse>>>,
    requests: Arc<Mutex<Vec<ChatCompletionRequest>>>,
}

impl DynamicListener {
    /// Create a new dynamic listener and a sender for queuing responses
    pub fn new() -> (Self, mpsc::Sender<MockAiResponse>) {
        let (tx, rx) = mpsc::channel(100);
        (
            Self {
                response_rx: Arc::new(tokio::sync::Mutex::new(rx)),
                requests: Arc::new(Mutex::new(Vec::new())),
            },
            tx,
        )
    }

    /// Get all recorded requests
    pub fn get_requests(&self) -> Vec<ChatCompletionRequest> {
        self.requests.lock().unwrap().clone()
    }

    /// Get the last request
    pub fn last_request(&self) -> Option<ChatCompletionRequest> {
        self.requests.lock().unwrap().last().cloned()
    }
}

impl MockAiListener for DynamicListener {
    fn on_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Pin<Box<dyn Future<Output = MockAiResponse> + Send>> {
        self.requests.lock().unwrap().push(request);
        let rx = self.response_rx.clone();
        Box::pin(async move {
            let mut guard = rx.lock().await;
            guard.recv().await.unwrap_or_else(|| {
                MockAiResponse::text("No response available")
            })
        })
    }
}
