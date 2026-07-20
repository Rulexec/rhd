use reqwest::Client;

use super::types::*;
use super::AiError;

pub(crate) struct ChatExecutor {
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    pub(crate) http: Client,
}

impl ChatExecutor {
    pub(crate) async fn chat_with_tools(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tools: &[ToolDefinition],
        mut raw_log: Option<&mut dyn RawLogger>,
    ) -> Result<ChatResult, AiError> {
        let url = format!("{}/chat/completions", self.base_url);

        let request = ChatRequest {
            model,
            messages,
            tools: if tools.is_empty() { None } else { Some(tools) },
            stream: false,
        };

        if let Some(ref mut logger) = raw_log {
            if let Ok(json) = serde_json::to_string_pretty(&request) {
                logger.log_request(&json);
            }
        }

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
            if let Some(ref mut logger) = raw_log {
                logger.log_error(Some(status.as_u16()), &body);
            }
            return Err(AiError::Api {
                model: model.to_string(),
                status: status.as_u16(),
                body,
            });
        }

        let response_text = response.text().await.map_err(|source| {
            AiError::Parse {
                model: model.to_string(),
                source,
            }
        })?;

        if let Some(ref mut logger) = raw_log {
            logger.log_response(&response_text);
        }

        let chat_response: ChatResponse = serde_json::from_str(&response_text).map_err(|source| {
            AiError::JsonParse {
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
            .enumerate()
            .map(|(idx, tc)| ToolCall {
                id: if tc.id.is_empty() {
                    format!("call_{}", idx)
                } else {
                    tc.id
                },
                call_type: tc.call_type,
                function: FunctionCall {
                    name: tc.function.name,
                    arguments: tc.function.arguments,
                },
            })
            .collect();

        let usage = chat_response.usage.map(|u| rhd_api::TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        });

        Ok(ChatResult {
            content: choice.message.content,
            tool_calls,
            finish_reason: choice.finish_reason,
            usage,
        })
    }
}
