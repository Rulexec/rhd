use std::sync::Arc;

use rhd_ai::client::{ChatMessage, OpenAiClient, ToolCall};
use rhd_ai::{FunctionDefinition, ToolDefinition};
use rhd_db::{ChatDb, Message};
use rhd_mcp_client::client::McpClient;
use rhd_mcp_client::McpClientTrait;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::chat_log::ChatLogSink;
use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::ProjectProvider;

pub async fn collect_tools_from_projects<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
) -> (Vec<ToolDefinition>, Vec<(String, String, Arc<McpClient>)>) {
    let mut tools = Vec::new();
    let mut mcp_clients = Vec::new();

    let attached_projects = match db.get_chat_projects(chat_id) {
        Ok(projects) => projects,
        Err(_) => return (tools, mcp_clients),
    };

    for (project_name, _) in attached_projects {
        let clients = project_provider.get_mcp_clients(&project_name).await;
        for (mcp_id, client) in clients {
            if let Ok(client_tools) = client.list_tools().await {
                for tool in client_tools {
                    let prefixed_name = format!("{}/{}", mcp_id, tool.name);
                    tools.push(ToolDefinition {
                        tool_type: "function".to_string(),
                        function: FunctionDefinition {
                            name: prefixed_name,
                            description: tool.description,
                            parameters: tool.input_schema,
                        },
                    });
                }
                mcp_clients.push((project_name.clone(), mcp_id, client));
            }
        }
    }

    (tools, mcp_clients)
}

pub async fn tool_loop<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    model: &str,
    api_model: &str,
    client: &OpenAiClient,
    tools: &[ToolDefinition],
    mcp_clients: &[(String, String, Arc<McpClient>)],
    cancel_token: &CancellationToken,
    event_sender: &broadcast::Sender<ChatEvent>,
    iterations: &mut u32,
    current_content: &mut String,
    max_iterations: u32,
    mut chat_log: Option<ChatLogSink>,
) -> Result<i64, ChatError> {
    loop {
        if cancel_token.is_cancelled() {
            if let Some(ref mut sink) = chat_log {
                sink.log_stream_error("aborted");
            }
            let _ = event_sender.send(ChatEvent::StreamError {
                chat_id,
                error: "aborted".to_string(),
            });
            return Err(ChatError::Ai(rhd_ai::client::AiError::Aborted {
                model: model.to_string(),
            }));
        }

        if *iterations >= max_iterations {
            return Err(ChatError::Ai(rhd_ai::client::AiError::Api {
                model: model.to_string(),
                status: 0,
                body: format!("max tool iterations ({}) exceeded", max_iterations),
            }));
        }

        manager.check_pause_state(chat_id, event_sender).await;

        let db_messages = manager.db().get_messages(chat_id)?;
        let chat_messages = build_chat_messages_for_tools(&db_messages);

        let result = client
            .chat_with_tools(api_model, chat_messages, tools)
            .await?;

        if result.tool_calls.is_empty() {
            let final_content = result.content.unwrap_or_default();
            let finish_reason = result.finish_reason.unwrap_or_else(|| "stop".to_string());
            
            if let Some(ref mut sink) = chat_log {
                sink.log_assistant_response(None, &final_content, &finish_reason, result.usage.as_ref());
                sink.log_stream_finished(&finish_reason, 0);
            }
            
            let assistant_message_id =
                manager.db().add_message(chat_id, "assistant", &final_content, Some(model), None)?;
            let assistant_message = Message {
                id: assistant_message_id,
                chat_id,
                role: "assistant".to_string(),
                content: final_content,
                created_at: chrono::Utc::now().to_rfc3339(),
                model: Some(model.to_string()),
                thinking_content: None,
            };
            let _ = event_sender.send(ChatEvent::MessageAdded {
                chat_id,
                message: assistant_message,
            });
            let _ = event_sender.send(ChatEvent::StreamFinished {
                chat_id,
                message_id: assistant_message_id,
                finish_reason,
            });
            return Ok(assistant_message_id);
        }

        *iterations += 1;

        let assistant_content = result.content.clone().unwrap_or_default();
        let assistant_msg_content = serde_json::json!({
            "content": assistant_content,
            "toolCalls": result.tool_calls,
        })
        .to_string();
        manager.db().add_message(
            chat_id,
            "assistant",
            &assistant_msg_content,
            Some(model),
            None,
        )?;

        for tool_call in &result.tool_calls {
            if cancel_token.is_cancelled() {
                if let Some(ref mut sink) = chat_log {
                    sink.log_stream_error("aborted");
                }
                let _ = event_sender.send(ChatEvent::StreamError {
                    chat_id,
                    error: "aborted".to_string(),
                });
                return Err(ChatError::Ai(rhd_ai::client::AiError::Aborted {
                    model: model.to_string(),
                }));
            }

            let mcp_id = extract_mcp_id_from_tool_name(&tool_call.name);

            if let Some(ref mut sink) = chat_log {
                sink.log_tool_call(&tool_call.name, &tool_call.id, &tool_call.arguments);
            }

            let _ = event_sender.send(ChatEvent::ToolCallStarted {
                chat_id,
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                arguments: tool_call.arguments.clone(),
                mcp_id: mcp_id.clone(),
            });

            let (tool_result, _) = execute_tool_call(tool_call, mcp_clients).await;

            if let Some(ref mut sink) = chat_log {
                sink.log_tool_result(&tool_call.name, &tool_call.id, &tool_result);
            }

            let _ = event_sender.send(ChatEvent::ToolCallCompleted {
                chat_id,
                tool_call_id: tool_call.id.clone(),
                result: tool_result.clone(),
            });

            let tool_result_json = serde_json::json!({
                "toolCallId": tool_call.id,
                "name": tool_call.name,
                "result": tool_result,
            })
            .to_string();
            manager.db().add_message(chat_id, "tool", &tool_result_json, None, None)?;
        }

        if let Some(content) = result.content {
            *current_content = content;
        }
    }
}

pub async fn execute_tool_call(
    tool_call: &ToolCall,
    mcp_clients: &[(String, String, Arc<McpClient>)],
) -> (String, String) {
    let (mcp_id, bare_tool_name) = split_tool_name(&tool_call.name);
    for (_project_name, client_mcp_id, client) in mcp_clients {
        if *client_mcp_id == mcp_id {
            match client.call_tool(&bare_tool_name, &tool_call.arguments).await {
                Ok(result) => return (result.content, client_mcp_id.clone()),
                Err(e) => return (format!("Error: {}", e), client_mcp_id.clone()),
            }
        }
    }
    (format!("Error: unknown tool '{}'", tool_call.name), String::new())
}

pub fn extract_mcp_id_from_tool_name(tool_name: &str) -> String {
    tool_name.split('/').next().unwrap_or("").to_string()
}

pub fn split_tool_name(tool_name: &str) -> (String, String) {
    if let Some((mcp_id, bare_name)) = tool_name.split_once('/') {
        (mcp_id.to_string(), bare_name.to_string())
    } else {
        (String::new(), tool_name.to_string())
    }
}

pub fn build_chat_messages(messages: &[Message]) -> Vec<ChatMessage> {
    messages
        .iter()
        .map(|m| match m.role.as_str() {
            "user" => ChatMessage::user(&m.content),
            "assistant" => ChatMessage::assistant(&m.content),
            "system" => ChatMessage::system(&m.content),
            _ => ChatMessage::user(&m.content),
        })
        .collect()
}

pub fn build_chat_messages_for_tools(messages: &[Message]) -> Vec<ChatMessage> {
    let mut chat_messages = Vec::new();
    
    for m in messages {
        match m.role.as_str() {
            "user" => chat_messages.push(ChatMessage::user(&m.content)),
            "system" => chat_messages.push(ChatMessage::system(&m.content)),
            "assistant" => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&m.content) {
                    let content = json.get("content").and_then(|c| c.as_str()).map(|s| s.to_string());
                    let tool_calls: Vec<ToolCall> = json.get("toolCalls")
                        .and_then(|tc| serde_json::from_value(tc.clone()).ok())
                        .unwrap_or_default();
                    
                    if tool_calls.is_empty() {
                        chat_messages.push(ChatMessage::assistant(content.unwrap_or_default()));
                    } else {
                        chat_messages.push(ChatMessage::assistant_with_tool_calls(content, tool_calls));
                    }
                } else {
                    chat_messages.push(ChatMessage::assistant(&m.content));
                }
            }
            "tool" => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&m.content) {
                    let tool_call_id = json.get("toolCallId")
                        .and_then(|id| id.as_str())
                        .unwrap_or("");
                    let result = json.get("result")
                        .and_then(|r| r.as_str())
                        .unwrap_or("");
                    chat_messages.push(ChatMessage::tool(tool_call_id, result));
                }
            }
            _ => chat_messages.push(ChatMessage::user(&m.content)),
        }
    }
    
    chat_messages
}
