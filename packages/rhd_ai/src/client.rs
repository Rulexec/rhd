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
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [ToolDefinition]>,
}

#[derive(Serialize)]
#[serde(untagged)]
#[allow(dead_code)]
enum ChatMessage<'a> {
    System { role: &'a str, content: &'a str },
    User { role: &'a str, content: &'a str },
    Assistant {
        role: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<&'a [ToolCall]>,
    },
    Tool {
        role: &'a str,
        tool_call_id: &'a str,
        content: &'a str,
    },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallResponse>>,
}

#[derive(Deserialize, Clone)]
struct ToolCallResponse {
    id: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    call_type: String,
    function: FunctionCall,
}

#[derive(Deserialize, Clone)]
struct FunctionCall {
    name: String,
    arguments: String,
}

pub struct ChatResult {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<String>,
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
        let result = self.chat_with_tools(model, system, message, &[], &[]).await?;
        Ok(result.content.unwrap_or_default())
    }

    pub async fn chat_with_tools(
        &self,
        model: &str,
        system: &str,
        message: &str,
        tools: &[ToolDefinition],
        tool_results: &[(String, String)], // (tool_call_id, content)
    ) -> Result<ChatResult, AiError> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut messages = vec![
            ChatMessage::System { role: "system", content: system },
            ChatMessage::User { role: "user", content: message },
        ];

        // Add tool results if any
        for (tool_call_id, content) in tool_results {
            messages.push(ChatMessage::Tool {
                role: "tool",
                tool_call_id,
                content,
            });
        }

        let request = ChatRequest {
            model,
            messages,
            tools: if tools.is_empty() { None } else { Some(tools) },
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

        let choice = chat_response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| AiError::NoChoices {
                model: model.to_string(),
            })?;

        let tool_calls = choice
            .message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tc| ToolCall {
                id: tc.id,
                name: tc.function.name,
                arguments: tc.function.arguments,
            })
            .collect();

        Ok(ChatResult {
            content: choice.message.content,
            tool_calls,
            finish_reason: choice.finish_reason,
        })
    }
}
