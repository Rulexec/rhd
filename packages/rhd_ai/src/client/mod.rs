mod chat;
mod stream;
mod types;

use reqwest::Client;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use chat::ChatExecutor;
use stream::StreamExecutor;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("network error calling {model}: {source}")]
    Network {
        model: String,
        source: reqwest::Error,
    },

    #[error("api error for {model}: status {status}, body: {body}")]
    Api {
        model: String,
        status: u16,
        body: String,
    },

    #[error("failed to parse response for {model}: {source}")]
    Parse {
        model: String,
        source: reqwest::Error,
    },

    #[error("failed to parse JSON for {model}: {source}")]
    JsonParse {
        model: String,
        source: serde_json::Error,
    },

    #[error("no choices in response for {model}")]
    NoChoices { model: String },

    #[error("streaming aborted for {model}")]
    Aborted { model: String },

    #[error("SSE stream error for {model} at event {event_index}: {message}")]
    StreamError {
        model: String,
        event_index: usize,
        message: String,
        source: reqwest::Error,
    },
}

pub struct OpenAiClient {
    base_url: String,
    api_key: String,
    http: Client,
}

impl OpenAiClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            http: Client::new(),
        }
    }

    pub async fn chat(
        &self,
        model: &str,
        system: &str,
        message: &str,
    ) -> Result<String, AiError> {
        let messages = vec![ChatMessage::system(system), ChatMessage::user(message)];
        let result = self.chat_with_tools(model, messages, &[], None).await?;
        Ok(result.content.unwrap_or_default())
    }

    pub async fn chat_with_tools(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: &[ToolDefinition],
        raw_log: Option<&mut dyn RawLogger>,
    ) -> Result<ChatResult, AiError> {
        let executor = ChatExecutor {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            http: self.http.clone(),
        };
        executor.chat_with_tools(model, messages, tools, raw_log).await
    }

    pub async fn chat_stream<F>(
        &self,
        model: &str,
        messages: &[ChatMessage],
        on_chunk: F,
        raw_log: Option<&mut dyn RawLogger>,
    ) -> Result<StreamResult, AiError>
    where
        F: FnMut(StreamChunk) -> bool,
    {
        let executor = StreamExecutor {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            http: self.http.clone(),
        };
        executor.chat_stream(model, messages, on_chunk, raw_log).await
    }

    pub async fn chat_stream_cancellable(
        &self,
        model: &str,
        messages: &[ChatMessage],
        cancel: CancellationToken,
        on_chunk: impl FnMut(StreamChunk) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
        raw_log: Option<&mut dyn RawLogger>,
    ) -> Result<StreamResult, AiError> {
        let executor = StreamExecutor {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            http: self.http.clone(),
        };
        executor.chat_stream_cancellable(model, messages, cancel, on_chunk, raw_log).await
    }

    pub async fn chat_stream_with_tools(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        cancel: CancellationToken,
        on_chunk: impl FnMut(StreamChunk) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
        raw_log: Option<&mut dyn RawLogger>,
    ) -> Result<StreamResultWithTools, AiError> {
        let executor = StreamExecutor {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            http: self.http.clone(),
        };
        executor.chat_stream_with_tools(model, messages, tools, cancel, on_chunk, raw_log).await
    }
}

pub use types::{
    ChatMessage, ChatResult, FunctionCall, FunctionDefinition, RawLogger, StreamChunk,
    StreamResult, StreamResultWithTools, ToolCall, ToolDefinition,
};
