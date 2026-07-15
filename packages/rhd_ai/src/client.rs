use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

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

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [ToolDefinition]>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum ChatMessage {
    System { role: String, content: String },
    User { role: String, content: String },
    Assistant {
        role: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning_content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ToolCall>>,
    },
    Tool {
        role: String,
        tool_call_id: String,
        content: String,
    },
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        ChatMessage::User {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        ChatMessage::System {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        ChatMessage::Assistant {
            role: "assistant".to_string(),
            content: Some(content.into()),
            reasoning_content: None,
            tool_calls: None,
        }
    }

    pub fn assistant_with_thinking(content: impl Into<String>, thinking: Option<String>) -> Self {
        ChatMessage::Assistant {
            role: "assistant".to_string(),
            content: Some(content.into()),
            reasoning_content: thinking,
            tool_calls: None,
        }
    }

    pub fn assistant_with_tool_calls(
        content: Option<String>,
        reasoning_content: Option<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        ChatMessage::Assistant {
            role: "assistant".to_string(),
            content,
            reasoning_content,
            tool_calls: if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            },
        }
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        ChatMessage::Tool {
            role: "tool".to_string(),
            tool_call_id: tool_call_id.into(),
            content: content.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
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
    call_type: String,
    function: FunctionCall,
}

pub struct ChatResult {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: Option<String>,
    pub usage: Option<rhd_api::TokenUsage>,
}

#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StreamResult {
    pub finish_reason: Option<String>,
    pub usage: Option<rhd_api::TokenUsage>,
}

#[derive(Deserialize)]
struct StreamResponse {
    choices: Vec<StreamChoice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct StreamDelta {
    content: Option<String>,
    #[serde(alias = "reasoning")]
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Deserialize, Clone)]
struct ToolCallDelta {
    index: usize,
    id: Option<String>,
    #[serde(rename = "type")]
    call_type: Option<String>,
    function: Option<FunctionCallDelta>,
}

#[derive(Deserialize, Clone)]
struct FunctionCallDelta {
    name: Option<String>,
    arguments: Option<String>,
}

pub struct StreamResultWithTools {
    pub finish_reason: Option<String>,
    pub usage: Option<rhd_api::TokenUsage>,
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

pub trait RawLogger: Send {
    fn log_request(&mut self, request_json: &str);
    fn log_stream_chunk(&mut self, index: usize, chunk_json: &str);
    fn log_response(&mut self, response_json: &str);
    fn log_error(&mut self, status: Option<u16>, body: &str);
    fn log_tool_result_raw(&mut self, tool_name: &str, call_id: &str, raw_json: &str);
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

    pub async fn chat_stream<F>(
        &self,
        model: &str,
        messages: &[ChatMessage],
        mut on_chunk: F,
        mut raw_log: Option<&mut dyn RawLogger>,
    ) -> Result<StreamResult, AiError>
    where
        F: FnMut(StreamChunk) -> bool,
    {
        let url = format!("{}/chat/completions", self.base_url);

        let request = ChatRequest {
            model,
            messages: messages.to_vec(),
            tools: None,
            stream: true,
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

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut finish_reason = None;
        let mut usage = None;
        let mut event_index = 0usize;

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|source| AiError::Network {
                model: model.to_string(),
                source,
            })?;

            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete SSE events (separated by \n\n)
            while let Some(event_end) = buffer.find("\n\n") {
                let event = buffer[..event_end].to_string();
                buffer = buffer[event_end + 2..].to_string();

                for line in event.lines() {
                    if line.starts_with("data: ") {
                        let data = &line[6..];

                        if data == "[DONE]" {
                            return Ok(StreamResult {
                                finish_reason,
                                usage,
                            });
                        }

                        if let Some(ref mut logger) = raw_log {
                            logger.log_stream_chunk(event_index, data);
                        }

                        let stream_response: StreamResponse =
                            serde_json::from_str(data).map_err(|source| AiError::JsonParse {
                                model: model.to_string(),
                                source,
                            })?;

                        if let Some(choice) = stream_response.choices.into_iter().next() {
                            let content = choice.delta.content;
                            let reasoning_content = choice.delta.reasoning_content;
                            finish_reason = choice.finish_reason.or(finish_reason);

                            let stream_chunk = StreamChunk {
                                content,
                                reasoning_content,
                                finish_reason: None,
                            };

                            if !on_chunk(stream_chunk) {
                                return Err(AiError::Aborted {
                                    model: model.to_string(),
                                });
                            }
                        }

                        if let Some(u) = stream_response.usage {
                            usage = Some(rhd_api::TokenUsage {
                                prompt_tokens: u.prompt_tokens,
                                completion_tokens: u.completion_tokens,
                                total_tokens: u.total_tokens,
                            });
                        }
                    }
                }
                event_index += 1;
            }
        }

        Ok(StreamResult {
            finish_reason,
            usage,
        })
    }

    pub async fn chat_stream_cancellable(
        &self,
        model: &str,
        messages: &[ChatMessage],
        cancel: CancellationToken,
        mut on_chunk: impl FnMut(StreamChunk) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
        mut raw_log: Option<&mut dyn RawLogger>,
    ) -> Result<StreamResult, AiError> {
        let url = format!("{}/chat/completions", self.base_url);

        let request = ChatRequest {
            model,
            messages: messages.to_vec(),
            tools: None,
            stream: true,
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

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut finish_reason = None;
        let mut usage = None;
        let mut event_index = 0usize;

        while let Some(chunk_result) = stream.next().await {
            if cancel.is_cancelled() {
                return Err(AiError::Aborted {
                    model: model.to_string(),
                });
            }

            let chunk = chunk_result.map_err(|source| AiError::StreamError {
                model: model.to_string(),
                event_index,
                message: format!("Failed to read SSE chunk: {}", source),
                source,
            })?;

            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete SSE events (separated by \n\n)
            while let Some(event_end) = buffer.find("\n\n") {
                let event = buffer[..event_end].to_string();
                buffer = buffer[event_end + 2..].to_string();

                for line in event.lines() {
                    if line.starts_with("data: ") {
                        let data = &line[6..];

                        if data == "[DONE]" {
                            return Ok(StreamResult {
                                finish_reason,
                                usage,
                            });
                        }

                        if let Some(ref mut logger) = raw_log {
                            logger.log_stream_chunk(event_index, data);
                        }

                        let stream_response: StreamResponse =
                            serde_json::from_str(data).map_err(|source| AiError::JsonParse {
                                model: model.to_string(),
                                source,
                            })?;

                        if let Some(choice) = stream_response.choices.into_iter().next() {
                            let content = choice.delta.content;
                            let reasoning_content = choice.delta.reasoning_content;
                            finish_reason = choice.finish_reason.or(finish_reason);

                            let stream_chunk = StreamChunk {
                                content,
                                reasoning_content,
                                finish_reason: None,
                            };

                            on_chunk(stream_chunk).await;
                        }

                        if let Some(u) = stream_response.usage {
                            usage = Some(rhd_api::TokenUsage {
                                prompt_tokens: u.prompt_tokens,
                                completion_tokens: u.completion_tokens,
                                total_tokens: u.total_tokens,
                            });
                        }
                    }
                }
                event_index += 1;
            }
        }

        Ok(StreamResult {
            finish_reason,
            usage,
        })
    }

    pub async fn chat_stream_with_tools(
        &self,
        model: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        cancel: CancellationToken,
        mut on_chunk: impl FnMut(StreamChunk) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
        mut raw_log: Option<&mut dyn RawLogger>,
    ) -> Result<StreamResultWithTools, AiError> {
        let url = format!("{}/chat/completions", self.base_url);

        let request = ChatRequest {
            model,
            messages: messages.to_vec(),
            tools: if tools.is_empty() { None } else { Some(tools) },
            stream: true,
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

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut finish_reason = None;
        let mut usage = None;
        let mut event_index = 0usize;
        let mut accumulated_content = String::new();
        let mut tool_call_accumulators: Vec<ToolCallAccumulator> = Vec::new();

        while let Some(chunk_result) = stream.next().await {
            if cancel.is_cancelled() {
                return Err(AiError::Aborted {
                    model: model.to_string(),
                });
            }

            let chunk = chunk_result.map_err(|source| AiError::StreamError {
                model: model.to_string(),
                event_index,
                message: format!("Failed to read SSE chunk: {}", source),
                source,
            })?;

            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete SSE events (separated by \n\n)
            while let Some(event_end) = buffer.find("\n\n") {
                let event = buffer[..event_end].to_string();
                buffer = buffer[event_end + 2..].to_string();

                for line in event.lines() {
                    if line.starts_with("data: ") {
                        let data = &line[6..];

                        if data == "[DONE]" {
                            let tool_calls = tool_call_accumulators
                                .into_iter()
                                .enumerate()
                                .filter_map(|(idx, acc)| acc.build(idx))
                                .collect();

                            return Ok(StreamResultWithTools {
                                finish_reason,
                                usage,
                                content: if accumulated_content.is_empty() {
                                    None
                                } else {
                                    Some(accumulated_content)
                                },
                                tool_calls,
                            });
                        }

                        if let Some(ref mut logger) = raw_log {
                            logger.log_stream_chunk(event_index, data);
                        }

                        let stream_response: StreamResponse =
                            serde_json::from_str(data).map_err(|source| AiError::JsonParse {
                                model: model.to_string(),
                                source,
                            })?;

                        if let Some(choice) = stream_response.choices.into_iter().next() {
                            let content = choice.delta.content;
                            let reasoning_content = choice.delta.reasoning_content;
                            finish_reason = choice.finish_reason.or(finish_reason);

                            if let Some(ref c) = content {
                                accumulated_content.push_str(c);
                            }

                            let stream_chunk = StreamChunk {
                                content,
                                reasoning_content,
                                finish_reason: None,
                            };

                            on_chunk(stream_chunk).await;

                            // Accumulate tool call deltas
                            if let Some(tool_call_deltas) = choice.delta.tool_calls {
                                for delta in tool_call_deltas {
                                    let index = delta.index;
                                    
                                    // Ensure we have an accumulator for this index
                                    while tool_call_accumulators.len() <= index {
                                        tool_call_accumulators.push(ToolCallAccumulator::default());
                                    }

                                    let acc = &mut tool_call_accumulators[index];
                                    
                                    if let Some(id) = delta.id {
                                        acc.id = Some(id);
                                    }
                                    if let Some(call_type) = delta.call_type {
                                        acc.call_type = Some(call_type);
                                    }
                                    if let Some(function_delta) = delta.function {
                                        if let Some(name) = function_delta.name {
                                            acc.function_name = Some(name);
                                        }
                                        if let Some(arguments) = function_delta.arguments {
                                            acc.arguments.push_str(&arguments);
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(u) = stream_response.usage {
                            usage = Some(rhd_api::TokenUsage {
                                prompt_tokens: u.prompt_tokens,
                                completion_tokens: u.completion_tokens,
                                total_tokens: u.total_tokens,
                            });
                        }
                    }
                }
                event_index += 1;
            }
        }

        let tool_calls = tool_call_accumulators
            .into_iter()
            .enumerate()
            .filter_map(|(idx, acc)| acc.build(idx))
            .collect();

        Ok(StreamResultWithTools {
            finish_reason,
            usage,
            content: if accumulated_content.is_empty() {
                None
            } else {
                Some(accumulated_content)
            },
            tool_calls,
        })
    }
}

#[derive(Default)]
struct ToolCallAccumulator {
    id: Option<String>,
    call_type: Option<String>,
    function_name: Option<String>,
    arguments: String,
}

impl ToolCallAccumulator {
    fn build(self, index: usize) -> Option<ToolCall> {
        let id = self.id.filter(|s| !s.is_empty()).unwrap_or_else(|| format!("call_{}", index));
        let call_type = self.call_type.unwrap_or_else(|| "function".to_string());
        let function_name = self.function_name?;

        Some(ToolCall {
            id,
            call_type,
            function: FunctionCall {
                name: function_name,
                arguments: self.arguments,
            },
        })
    }
}
