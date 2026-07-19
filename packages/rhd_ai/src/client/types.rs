use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct ChatRequest<'a> {
    pub model: &'a str,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<&'a [ToolDefinition]>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stream: bool,
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
pub(crate) struct ChatResponse {
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Deserialize)]
pub(crate) struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Deserialize)]
pub(crate) struct Choice {
    pub message: ResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct ResponseMessage {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallResponse>>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct ToolCallResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
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
pub(crate) struct StreamResponse {
    pub choices: Vec<StreamChoice>,
    pub usage: Option<Usage>,
}

#[derive(Deserialize)]
pub(crate) struct StreamChoice {
    pub delta: StreamDelta,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct StreamDelta {
    pub content: Option<String>,
    #[serde(alias = "reasoning")]
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub call_type: Option<String>,
    pub function: Option<FunctionCallDelta>,
}

#[derive(Deserialize, Clone)]
pub(crate) struct FunctionCallDelta {
    pub name: Option<String>,
    pub arguments: Option<String>,
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
