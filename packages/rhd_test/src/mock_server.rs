use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

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
pub type StreamChunkSender = Arc<Mutex<Option<mpsc::Sender<Option<String>>>>>;
pub type SharedAutoStream = Arc<Mutex<bool>>;

pub async fn chat_completions(
    State((requests, response, flag_value, stream_sender_holder, auto_stream)): State<(
        SharedRequests,
        SharedResponse,
        SharedFlagValue,
        StreamChunkSender,
        SharedAutoStream,
    )>,
    Json(body): Json<ChatRequest>,
) -> impl IntoResponse {
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
    let should_call_tool = has_tools && !has_tool_result;
    
    requests.lock().unwrap().push(RecordedRequest {
        model: body.model.clone(),
        system_content,
        user_content,
    });

    let response_content = response.lock().unwrap().clone();

    if body.stream {
        // Streaming response
        if should_call_tool {
            // Tool calls not supported in streaming for simplicity, return empty content
            let events: Vec<Result<Event, Infallible>> = vec![
                Ok(Event::default().data(r#"{"choices":[{"delta":{"content":""},"finish_reason":null}]}"#)),
                Ok(Event::default().data("[DONE]")),
            ];
            let stream = futures_util::stream::iter(events.into_iter());
            Sse::new(stream)
                .keep_alive(KeepAlive::new().interval(Duration::from_secs(1)))
                .into_response()
        } else {
            let use_auto_stream = *auto_stream.lock().unwrap();
            
            if use_auto_stream {
                // Auto-stream mode: send configured response immediately
                let content = response_content.clone();
                let events: Vec<Result<Event, Infallible>> = vec![
                    Ok(Event::default().data(format!(r#"{{"choices":[{{"delta":{{"content":"{}"}},"finish_reason":null}}]}}"#, content))),
                    Ok(Event::default().data(r#"{"choices":[{"delta":{"content":null},"finish_reason":"stop"}]}"#)),
                    Ok(Event::default().data("[DONE]")),
                ];
                let stream = futures_util::stream::iter(events.into_iter());
                Sse::new(stream)
                    .keep_alive(KeepAlive::new().interval(Duration::from_secs(1)))
                    .into_response()
            } else {
                // Manual control mode: wait for control server chunks
                // Create channel for controlling stream content
                let (tx, mut rx) = mpsc::channel::<Option<String>>(100);
                
                // Store sender so control server can send chunks
                *stream_sender_holder.lock().unwrap() = Some(tx);
                
                // Create channel for SSE events
                let (event_tx, event_rx) = mpsc::channel::<Result<Event, Infallible>>(100);
                
                // Spawn task to convert control chunks to SSE events
                tokio::spawn(async move {
                    loop {
                        match rx.recv().await {
                            Some(chunk) => {
                                match chunk {
                                    Some(content) => {
                                        let stream_resp = StreamResponse {
                                            choices: vec![StreamChoice {
                                                delta: StreamDelta { content: Some(content) },
                                                finish_reason: None,
                                            }],
                                        };
                                        let json = serde_json::to_string(&stream_resp).unwrap();
                                        if event_tx.send(Ok(Event::default().data(json))).await.is_err() {
                                            break;
                                        }
                                    }
                                    None => {
                                        // Stream finished
                                        let final_resp = StreamResponse {
                                            choices: vec![StreamChoice {
                                                delta: StreamDelta { content: None },
                                                finish_reason: Some("stop".to_string()),
                                            }],
                                        };
                                        let json = serde_json::to_string(&final_resp).unwrap();
                                        let _ = event_tx.send(Ok(Event::default().data(json))).await;
                                        let _ = event_tx.send(Ok(Event::default().data("[DONE]"))).await;
                                        break;
                                    }
                                }
                            }
                            None => break,
                        }
                    }
                });
                
                let stream = ReceiverStream::new(event_rx);
                Sse::new(stream)
                    .keep_alive(KeepAlive::new().interval(Duration::from_secs(1)))
                    .into_response()
            }
        }
    } else {
        // Non-streaming response
        let chat_response = if should_call_tool {
            // Check if MCP tools are present (tools with "/" in name like "mock1/echo")
            let tools = body.tools.as_ref().unwrap();
            let has_mcp_tools = tools.iter().any(|t| {
                t.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|n| n.contains('/'))
                    .unwrap_or(false)
            });

            let tool_call = if has_mcp_tools {
                // Return MCP tool call (mock1/echo)
                ToolCallResponse {
                    id: "call_1".to_string(),
                    call_type: "function".to_string(),
                    function: FunctionCallResponse {
                        name: "mock1/echo".to_string(),
                        arguments: r#"{"message":"Hello MCP"}"#.to_string(),
                    },
                }
            } else {
                // Return built-in tool call (rhd_set_flag)
                let expected_flag = *flag_value.lock().unwrap();
                let arguments = format!(r#"{{"name":"test_flag","value":{}}}"#, expected_flag);
                ToolCallResponse {
                    id: "call_1".to_string(),
                    call_type: "function".to_string(),
                    function: FunctionCallResponse {
                        name: "rhd_set_flag".to_string(),
                        arguments,
                    },
                }
            };

            ChatResponse {
                choices: vec![Choice {
                    message: ResponseMessage {
                        role: "assistant".to_string(),
                        content: None,
                        tool_calls: Some(vec![tool_call]),
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

pub async fn start_mock_server() -> (
    u16,
    SharedRequests,
    SharedResponse,
    SharedFlagValue,
    StreamChunkSender,
    SharedAutoStream,
) {
    let requests: SharedRequests = Arc::new(Mutex::new(Vec::new()));
    let response: SharedResponse = Arc::new(Mutex::new(String::new()));
    let flag_value: SharedFlagValue = Arc::new(Mutex::new(true));
    let stream_sender: StreamChunkSender = Arc::new(Mutex::new(None));
    let auto_stream: SharedAutoStream = Arc::new(Mutex::new(true));
    
    let app = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .with_state((
            requests.clone(),
            response.clone(),
            flag_value.clone(),
            stream_sender.clone(),
            auto_stream.clone(),
        ));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (port, requests, response, flag_value, stream_sender, auto_stream)
}
