use reqwest::Client;

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
    ) -> Result<impl futures_util::Stream<Item = Result<StreamChunk, AiClientError>>, AiClientError>
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

        // Read the entire response as text and parse SSE events
        let text = response.text().await?;
        let chunks = parse_sse_text(&text);

        Ok(futures_util::stream::iter(chunks))
    }

}

/// Parse SSE text into StreamChunks.
///
/// SSE format: each event starts with "data: " followed by JSON, ending with "\n\n".
/// Special "[DONE]" marker indicates end of stream.
fn parse_sse_text(text: &str) -> Vec<Result<StreamChunk, AiClientError>> {
    let mut chunks = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(':') {
            continue;
        }

        if let Some(data) = line.strip_prefix("data: ") {
            let data = data.trim();
            if data == "[DONE]" {
                break;
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
                        chunks.push(Ok(chunk));
                    }
                }
                Err(e) => {
                    chunks.push(Err(AiClientError::Stream(format!(
                        "failed to parse SSE data: {}",
                        e
                    ))));
                }
            }
        }
    }

    chunks
}
