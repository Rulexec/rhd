use std::sync::{Arc, Mutex};

use axum::{extract::State, response::{sse::{Event, Sse}, IntoResponse}, routing::post, Json, Router};
use futures_util::stream;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub stream: bool,
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

#[derive(Debug, Serialize)]
struct StreamChoice {
    delta: StreamDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct StreamDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
}

#[derive(Debug, Serialize)]
struct StreamResponse {
    choices: Vec<StreamChoice>,
}

pub type SharedRequests = Arc<Mutex<Vec<RecordedRequest>>>;
pub type SharedResponse = Arc<Mutex<String>>;
pub type SharedFlagValue = Arc<Mutex<bool>>;

pub async fn chat_completions(
    State((requests, response, flag_value)): State<(SharedRequests, SharedResponse, SharedFlagValue)>,
    Json(body): Json<ChatRequest>,
) -> impl axum::response::IntoResponse {
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

    if body.stream {
        // Streaming response
        let events: Vec<Event> = if should_call_tool {
            // Tool calls not supported in streaming for simplicity, return empty content
            vec![
                Event::default().data(r#"{"choices":[{"delta":{"content":""},"finish_reason":null}]}"#),
                Event::default().data("[DONE]"),
            ]
        } else {
            // Split response into chunks for streaming effect
            let mut events = Vec::new();
            let words: Vec<&str> = response_content.split_whitespace().collect();
            for (i, word) in words.iter().enumerate() {
                let content = if i == 0 {
                    word.to_string()
                } else {
                    format!(" {}", word)
                };
                let stream_resp = StreamResponse {
                    choices: vec![StreamChoice {
                        delta: StreamDelta { content: Some(content) },
                        finish_reason: None,
                    }],
                };
                let json = serde_json::to_string(&stream_resp).unwrap();
                events.push(Event::default().data(json));
            }
            // Final event with finish_reason
            let final_resp = StreamResponse {
                choices: vec![StreamChoice {
                    delta: StreamDelta { content: None },
                    finish_reason: Some("stop".to_string()),
                }],
            };
            let json = serde_json::to_string(&final_resp).unwrap();
            events.push(Event::default().data(json));
            events.push(Event::default().data("[DONE]"));
            events
        };
        Sse::new(stream::iter(events.into_iter().map(Ok::<_, std::convert::Infallible>))).into_response()
    } else {
        // Non-streaming response
        let chat_response = if should_call_tool {
            let expected_flag = *flag_value.lock().unwrap();
            let arguments = format!(r#"{{"name":"test_flag","value":{}}}"#, expected_flag);
            ChatResponse {
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
            }
        } else {
            ChatResponse {
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
            }
        };
        Json(chat_response).into_response()
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
