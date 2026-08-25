use futures_util::Stream;
use reqwest::Client;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::error::AiClientError;
use crate::types::*;

pub struct AiClient {
    base_url: String,
    api_key: String,
    http: Client,
}

impl AiClient {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into(),
            http: Client::new(),
        }
    }

    /// Non-streaming chat completion
    pub async fn chat_completion(
        &self,
        mut request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, AiClientError> {
        request.stream = false;
        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AiClientError::Api {
                status: status.as_u16(),
                body,
            });
        }

        let chat_response: ChatCompletionResponse = response.json().await?;
        
        if chat_response.choices.is_empty() {
            return Err(AiClientError::NoChoices);
        }

        Ok(chat_response)
    }

    /// Streaming chat completion.
    ///
    /// Returns a stream of chunks. Each chunk contains deltas for reasoning content,
    /// content, and tool calls.
    pub async fn chat_completion_stream(
        &self,
        mut request: ChatCompletionRequest,
    ) -> Result<impl Stream<Item = Result<StreamChunk, AiClientError>>, AiClientError> {
        request.stream = true;
        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AiClientError::Api {
                status: status.as_u16(),
                body,
            });
        }

        // Stream the response body and parse SSE events incrementally
        let byte_stream = response.bytes_stream();
        Ok(SseStream::new(byte_stream))
    }
}

/// A stream that parses Server-Sent Events from a byte stream incrementally.
///
/// This struct wraps a byte stream and yields `StreamChunk` objects as soon as
/// each SSE event is complete, enabling true streaming without buffering the
/// entire response.
struct SseStream<S> {
    inner: S,
    buffer: String,
    done: bool,
}

impl<S> SseStream<S> {
    fn new(inner: S) -> Self {
        Self {
            inner,
            buffer: String::new(),
            done: false,
        }
    }
}

impl<S> Stream for SseStream<S>
where
    S: Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<StreamChunk, AiClientError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            // Try to parse a complete SSE event from the buffer
            if let Some(event_result) = self.parse_next_event() {
                return Poll::Ready(Some(event_result));
            }

            // If we're done, return None
            if self.done {
                return Poll::Ready(None);
            }

            // Poll the inner stream for more bytes
            match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    // Convert bytes to string and append to buffer
                    match std::str::from_utf8(&bytes) {
                        Ok(text) => self.buffer.push_str(text),
                        Err(_) => {
                            // Invalid UTF-8, skip these bytes
                            continue;
                        }
                    }
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(AiClientError::Network(e))));
                }
                Poll::Ready(None) => {
                    // Stream ended, try to parse any remaining data
                    self.done = true;
                    if let Some(event_result) = self.parse_next_event() {
                        return Poll::Ready(Some(event_result));
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }
    }
}

impl<S> SseStream<S> {
    /// Try to parse the next complete SSE event from the buffer.
    ///
    /// Returns `Some(result)` if a complete event was parsed, `None` if we need more data.
    fn parse_next_event(&mut self) -> Option<Result<StreamChunk, AiClientError>> {
        // Look for a complete SSE event (ends with \n\n)
        let event_end = self.buffer.find("\n\n")?;
        
        // Extract the event text and update buffer in one go
        let event_text = self.buffer[..event_end].to_string();
        self.buffer = self.buffer[event_end + 2..].to_string();
        
        // Parse the event
        for line in event_text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                let data = data.trim();
                if data == "[DONE]" {
                    self.done = true;
                    return None;
                }

                match serde_json::from_str::<StreamResponse>(data) {
                    Ok(response) => {
                        if let Some(choice) = response.choices.first() {
                            let delta = &choice.delta;
                            let chunk = StreamChunk {
                                reasoning_content: delta.reasoning_content.clone(),
                                content: delta.content.clone(),
                                tool_calls: delta.tool_calls.clone(),
                                finish_reason: choice.finish_reason.clone(),
                            };
                            return Some(Ok(chunk));
                        }
                    }
                    Err(e) => {
                        return Some(Err(AiClientError::Stream(format!(
                            "failed to parse SSE data: {}",
                            e
                        ))));
                    }
                }
            }
        }

        None
    }
}

