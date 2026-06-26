use std::sync::{Arc, Mutex};

use axum::{extract::State, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default)]
    #[allow(dead_code)]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Debug, Serialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub message: ResponseMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResponseMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallResponse>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ToolCallResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCallResponse,
}

#[derive(Debug, Serialize, Clone)]
pub struct FunctionCallResponse {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct RecordedRequest {
    pub model: String,
    pub system_content: String,
    pub user_content: String,
}

pub type SharedRequests = Arc<Mutex<Vec<RecordedRequest>>>;
pub type SharedResponse = Arc<Mutex<String>>;
pub type SharedFlagValue = Arc<Mutex<bool>>;

pub async fn chat_completions(
    State((requests, response, flag_value)): State<(SharedRequests, SharedResponse, SharedFlagValue)>,
    Json(body): Json<ChatRequest>,
) -> Json<ChatResponse> {
    let system_content = body
        .messages
        .iter()
        .find(|m| m.role == "system")
        .and_then(|m| m.content.clone())
        .unwrap_or_default();
    let user_content = body
        .messages
        .iter()
        .find(|m| m.role == "user")
        .and_then(|m| m.content.clone())
        .unwrap_or_default();

    let has_tools = body.tools.is_some();
    let has_tool_result = body.messages.iter().any(|m| m.role == "tool");
    let should_call_tool = has_tools && system_content.contains("rhd_set_flag") && !has_tool_result;

    requests.lock().unwrap().push(RecordedRequest {
        model: body.model.clone(),
        system_content,
        user_content,
    });

    let response_content = response.lock().unwrap().clone();

    if should_call_tool {
        let expected_flag = *flag_value.lock().unwrap();
        let arguments = format!(r#"{{"name":"test_flag","value":{}}}"#, expected_flag);
        Json(ChatResponse {
            choices: vec![Choice {
                message: ResponseMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(vec![ToolCallResponse {
                        id: "call_1".to_string(),
                        call_type: "function".to_string(),
                        function: FunctionCallResponse {
                            name: "rhd_set_flag".to_string(),
                            arguments,
                        },
                    }]),
                },
                finish_reason: Some("tool_calls".to_string()),
            }],
            usage: Some(Usage {
                prompt_tokens: 100,
                completion_tokens: 20,
                total_tokens: 120,
            }),
        })
    } else {
        Json(ChatResponse {
            choices: vec![Choice {
                message: ResponseMessage {
                    role: "assistant".to_string(),
                    content: Some(response_content),
                    tool_calls: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Some(Usage {
                prompt_tokens: 50,
                completion_tokens: 10,
                total_tokens: 60,
            }),
        })
    }
}

pub async fn start_mock_server() -> (u16, SharedRequests, SharedResponse, SharedFlagValue) {
    let requests: SharedRequests = Arc::new(Mutex::new(Vec::new()));
    let response: SharedResponse = Arc::new(Mutex::new(String::new()));
    let flag_value: SharedFlagValue = Arc::new(Mutex::new(true));
    let app = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .with_state((requests.clone(), response.clone(), flag_value.clone()));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (port, requests, response, flag_value)
}
