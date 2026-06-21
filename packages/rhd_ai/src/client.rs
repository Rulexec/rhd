use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

    #[error("no choices in response for {model}")]
    NoChoices { model: String },
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
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
        let url = format!("{}/chat/completions", self.base_url);

        let request = ChatRequest {
            model,
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: system,
                },
                ChatMessage {
                    role: "user",
                    content: message,
                },
            ],
        };

        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|source| AiError::Network {
                model: model.to_string(),
                source,
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AiError::Api {
                model: model.to_string(),
                status: status.as_u16(),
                body,
            });
        }

        let chat_response: ChatResponse = response.json().await.map_err(|source| {
            AiError::Parse {
                model: model.to_string(),
                source,
            }
        })?;

        let content = chat_response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| AiError::NoChoices {
                model: model.to_string(),
            })?
            .message
            .content;

        Ok(content)
    }
}
