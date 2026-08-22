use reqwest::Client;
use futures_util::StreamExt;

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

    /// Streaming chat completion
    pub async fn chat_completion_stream<F>(
        &self,
        mut request: ChatCompletionRequest,
        mut on_chunk: F,
    ) -> Result<StreamResult, AiClientError>
    where
        F: FnMut(StreamChunk) -> bool,
    {
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

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut finish_reason = None;
        let mut usage = None;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AiClientError::Stream(e.to_string()))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete lines
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim().to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                // Check for [DONE] marker
                if line == "data: [DONE]" {
                    return Ok(StreamResult { finish_reason, usage });
                }

                // Parse SSE data
                if let Some(data) = line.strip_prefix("data: ") {
                    if let Ok(stream_response) = serde_json::from_str::<StreamResponse>(data) {
                        if let Some(choice) = stream_response.choices.first() {
                            let chunk = StreamChunk {
                                content: choice.delta.content.clone(),
                                tool_calls: choice.delta.tool_calls.clone(),
                                finish_reason: choice.finish_reason.clone(),
                            };

                            if choice.finish_reason.is_some() {
                                finish_reason = choice.finish_reason.clone();
                            }

                            if !on_chunk(chunk) {
                                return Ok(StreamResult { finish_reason, usage });
                            }
                        }

                        if let Some(u) = stream_response.usage {
                            usage = Some(u);
                        }
                    }
                }
            }
        }

        Ok(StreamResult { finish_reason, usage })
    }
}
