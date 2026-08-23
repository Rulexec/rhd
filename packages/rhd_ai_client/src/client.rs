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

}
