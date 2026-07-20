use std::time::Duration;
use std::convert::Infallible;

use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use super::types::*;

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
    // Check if the last message is a user message (not a tool result)
    // This allows triggering tool calls for each new user message
    let last_message_is_user = body.messages.last().map(|m| m.role == "user").unwrap_or(false);
    let should_call_tool = has_tools && last_message_is_user;
    
    requests.lock().unwrap().push(RecordedRequest {
        model: body.model.clone(),
        system_content,
        user_content,
    });

    let response_content = response.lock().unwrap().clone();

    if body.stream {
        // Streaming response
        if should_call_tool {
            // Check if MCP tools are present (tools with "/" in name like "mock1/echo")
            let tools = body.tools.as_ref().unwrap();
            let has_mcp_tools = tools.iter().any(|t| {
                t.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|n| n.contains('/'))
                    .unwrap_or(false)
            });

            let (tool_name, arguments) = if has_mcp_tools {
                ("mock1/echo".to_string(), r#"{"message":"Hello MCP"}"#.to_string())
            } else {
                let expected_flag = *flag_value.lock().unwrap();
                ("rhd_set_flag".to_string(), format!(r#"{{"name":"test_flag","value":{}}}"#, expected_flag))
            };

            let events: Vec<Result<Event, Infallible>> = vec![
                Ok(Event::default().data(serde_json::to_string(&StreamResponse {
                    choices: vec![StreamChoice {
                        delta: StreamDelta {
                            content: None,
                            tool_calls: Some(vec![StreamToolCallDelta {
                                index: 0,
                                id: Some("call_1".to_string()),
                                call_type: Some("function".to_string()),
                                function: Some(StreamFunctionCallDelta {
                                    name: Some(tool_name),
                                    arguments: Some(String::new()),
                                }),
                            }]),
                        },
                        finish_reason: None,
                    }],
                }).unwrap())),
                Ok(Event::default().data(serde_json::to_string(&StreamResponse {
                    choices: vec![StreamChoice {
                        delta: StreamDelta {
                            content: None,
                            tool_calls: Some(vec![StreamToolCallDelta {
                                index: 0,
                                id: None,
                                call_type: None,
                                function: Some(StreamFunctionCallDelta {
                                    name: None,
                                    arguments: Some(arguments),
                                }),
                            }]),
                        },
                        finish_reason: None,
                    }],
                }).unwrap())),
                Ok(Event::default().data(serde_json::to_string(&StreamResponse {
                    choices: vec![StreamChoice {
                        delta: StreamDelta {
                            content: None,
                            tool_calls: None,
                        },
                        finish_reason: Some("tool_calls".to_string()),
                    }],
                }).unwrap())),
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
                                                delta: StreamDelta { content: Some(content), tool_calls: None },
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
                                                delta: StreamDelta { content: None, tool_calls: None },
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
