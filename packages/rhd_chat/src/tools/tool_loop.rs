use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rhd_ai::client::{OpenAiClient, RawLogger, ToolCall};
use rhd_ai::ToolDefinition;
use rhd_db::ChatDb;
use rhd_mcp_client::client::McpClient;
use rhd_mcp_client::{McpClientTrait, ToolResult};
use tokio::sync::{broadcast, Mutex};
use tokio_util::sync::CancellationToken;

use crate::chat_log::ChatLoggers;
use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::state::PendingToolCall;
use crate::stream::TemplateLoaderRef;
use crate::ProjectProvider;

use super::builtin::{
    RHD_SET_ROLE_TOOL_NAME, RHD_SET_TODO_LIST_TOOL_NAME, handle_rhd_set_role,
    handle_rhd_set_todo_list, inject_todo_list_message,
};
use super::messages::build_chat_messages_for_tools;
use super::utils::{extract_mcp_id_from_tool_name, split_tool_name};

#[derive(Debug)]
pub enum ToolLoopResult {
    Completed { message_id: i64 },
    Paused,
}

pub async fn collect_tools_from_projects<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    template_loader: &TemplateLoaderRef,
) -> (Vec<ToolDefinition>, Vec<(String, String, Arc<McpClient>)>) {
    let mut tools = Vec::new();
    let mut mcp_clients = Vec::new();

    tools.extend(super::builtin::collect_builtin_tools(db, project_provider, chat_id, template_loader));

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
                        function: rhd_ai::FunctionDefinition {
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
    mut loggers: Option<ChatLoggers>,
    template_loader: &crate::stream::TemplateLoaderRef,
) -> Result<ToolLoopResult, ChatError> {
    loop {
        if cancel_token.is_cancelled() {
            if let Some(ref mut l) = loggers {
                l.chat_log.log_stream_error("aborted");
            }
            let _ = event_sender.send(ChatEvent::StreamAborted { chat_id });
            manager.abort_chat(chat_id, Vec::new()).await;
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

        crate::projects::inject_pending_role_prompt(manager, chat_id, event_sender, template_loader)?;

        let db_messages = manager.db().get_messages(chat_id)?;
        let chat_messages = build_chat_messages_for_tools(&db_messages);

        let raw_log_ref = loggers.as_mut().and_then(|l| l.raw_log.as_mut().map(|r| r as &mut dyn RawLogger));
        
        let accumulated_content = Arc::new(Mutex::new(String::new()));
        let accumulated_thinking = Arc::new(Mutex::new(String::new()));
        let accumulated_clone = accumulated_content.clone();
        let thinking_clone = accumulated_thinking.clone();
        let sender_for_closure = event_sender.clone();
        
        let result = client
            .chat_stream_with_tools(
                api_model,
                &chat_messages,
                tools,
                cancel_token.clone(),
                move |chunk| {
                    let sender = sender_for_closure.clone();
                    let acc = accumulated_clone.clone();
                    let thinking_acc = thinking_clone.clone();
                    Box::pin(async move {
                        if let Some(content) = chunk.content {
                            if !content.is_empty() {
                                let _ = sender.send(ChatEvent::StreamChunk {
                                    chat_id,
                                    content: content.clone(),
                                });
                                let mut acc_guard = acc.lock().await;
                                acc_guard.push_str(&content);
                            }
                        }
                        if let Some(reasoning) = chunk.reasoning_content {
                            if !reasoning.is_empty() {
                                let _ = sender.send(ChatEvent::ThinkingChunk {
                                    chat_id,
                                    content: reasoning.clone(),
                                });
                                let mut thinking_guard = thinking_acc.lock().await;
                                thinking_guard.push_str(&reasoning);
                            }
                        }
                    })
                },
                raw_log_ref,
            )
            .await;

        // Check if the AI call was aborted
        let result = match result {
            Ok(r) => r,
            Err(e) => {
                if let rhd_ai::client::AiError::Aborted { .. } = e {
                    // Emit StreamAborted event
                    let _ = event_sender.send(ChatEvent::StreamAborted { chat_id });
                    
                    // Transition to Aborted state
                    manager.abort_chat(chat_id, Vec::new()).await;
                    
                    return Err(ChatError::Ai(e));
                }
                return Err(ChatError::Ai(e));
            }
        };

        if result.tool_calls.is_empty() {
            let final_content: String = accumulated_content.lock().await.clone();
            let full_thinking: String = accumulated_thinking.lock().await.clone();
            let finish_reason = result.finish_reason.unwrap_or_else(|| "stop".to_string());
            
            if let Some(ref mut l) = loggers {
                let reasoning = if full_thinking.is_empty() {
                    None
                } else {
                    Some(full_thinking.as_str())
                };
                l.chat_log.log_assistant_response(reasoning, &final_content, &finish_reason, result.usage.as_ref());
                l.chat_log.log_stream_finished(&finish_reason, 0);
            }
            
            let thinking_option = if full_thinking.is_empty() {
                None
            } else {
                Some(full_thinking.clone())
            };
            
            let assistant_message = manager.add_message_and_notify(
                chat_id,
                "assistant",
                &final_content,
                Some(model),
                thinking_option.as_deref(),
                event_sender,
            )?;
            let assistant_message_id = assistant_message.id;
            let _ = event_sender.send(ChatEvent::StreamFinished {
                chat_id,
                message_id: assistant_message_id,
                finish_reason,
            });
            return Ok(ToolLoopResult::Completed { message_id: assistant_message_id });
        }

        *iterations += 1;

        // Make tool call IDs globally unique using atomic counter
        static TOOL_CALL_COUNTER: AtomicU64 = AtomicU64::new(0);
        let tool_calls_with_unique_ids: Vec<ToolCall> = result.tool_calls
            .into_iter()
            .map(|mut tc| {
                let unique_id = TOOL_CALL_COUNTER.fetch_add(1, Ordering::SeqCst);
                tc.id = format!("call_{}", unique_id);
                tc
            })
            .collect();

        let assistant_content: String = accumulated_content.lock().await.clone();
        let thinking_content: String = accumulated_thinking.lock().await.clone();
        let assistant_msg_content = serde_json::json!({
            "content": assistant_content,
            "toolCalls": tool_calls_with_unique_ids,
        })
        .to_string();
        let thinking_option = if thinking_content.is_empty() {
            None
        } else {
            Some(thinking_content.as_str())
        };
        
        // Emit ToolCallStarted events FIRST (before saving intermediate message)
        // This ensures frontend creates temp message before MessageAdded arrives
        for tool_call in &tool_calls_with_unique_ids {
            let mcp_id = extract_mcp_id_from_tool_name(&tool_call.function.name);

            if let Some(ref mut l) = loggers {
                l.chat_log.log_tool_call(&tool_call.function.name, &tool_call.id, &tool_call.function.arguments);
            }

            let _ = event_sender.send(ChatEvent::ToolCallStarted {
                chat_id,
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.function.name.clone(),
                arguments: tool_call.function.arguments.clone(),
                mcp_id: mcp_id.clone(),
            });
        }
        
        // Save intermediate assistant message (frontend will replace temp message created by ToolCallStarted)
        let intermediate_message = manager.add_message_and_notify(
            chat_id,
            "assistant",
            &assistant_msg_content,
            Some(model),
            thinking_option,
            event_sender,
        )?;
        let _intermediate_msg_id = intermediate_message.id;
        
        // Execute tools and send completion events
        let mut aborted_tool_ids = Vec::new();
        
        for tool_call in &tool_calls_with_unique_ids {
            // Check if cancelled before starting tool
            if cancel_token.is_cancelled() {
                aborted_tool_ids.push(tool_call.id.clone());
                
                let aborted_result = ToolResult {
                    content: "Aborted".to_string(),
                    is_error: Some(true),
                    raw_response: None,
                };
                
                let _ = event_sender.send(ChatEvent::ToolCallCompleted {
                    chat_id,
                    tool_call_id: tool_call.id.clone(),
                    result: aborted_result.content.clone(),
                    is_error: true,
                });
                
                let tool_result_json = serde_json::json!({
                    "toolCallId": tool_call.id,
                    "name": tool_call.function.name,
                    "result": aborted_result.content,
                    "isError": true,
                })
                .to_string();
                
                let _ = manager.add_message_and_notify(
                    chat_id,
                    "tool",
                    &tool_result_json,
                    None,
                    None,
                    event_sender,
                )?;
                
                continue;
            }

            let (tool_result, _) = execute_tool_call(manager, chat_id, tool_call, mcp_clients, event_sender).await;

            if let Some(ref mut l) = loggers {
                l.chat_log.log_tool_result(&tool_call.function.name, &tool_call.id, &tool_result.content);
            }

            if let Some(ref mut l) = loggers {
                if let Some(ref mut raw_log) = l.raw_log {
                    if let Some(ref raw_response) = tool_result.raw_response {
                        let raw_json = serde_json::to_string_pretty(raw_response).unwrap_or_else(|_| raw_response.to_string());
                        raw_log.log_tool_result_raw(&tool_call.function.name, &tool_call.id, &raw_json);
                    }
                }
            }

            let _ = event_sender.send(ChatEvent::ToolCallCompleted {
                chat_id,
                tool_call_id: tool_call.id.clone(),
                result: tool_result.content.clone(),
                is_error: tool_result.is_error.unwrap_or(false),
            });

            let tool_result_json = serde_json::json!({
                "toolCallId": tool_call.id,
                "name": tool_call.function.name,
                "result": tool_result.content,
                "isError": tool_result.is_error.unwrap_or(false),
            })
            .to_string();
            let tool_message = manager.add_message_and_notify(
                chat_id,
                "tool",
                &tool_result_json,
                None,
                None,
                event_sender,
            )?;
            let _tool_msg_id = tool_message.id;
        }
        
        // If any tools were aborted, transition to Aborted state
        if !aborted_tool_ids.is_empty() {
            manager.abort_chat(chat_id, aborted_tool_ids).await;
            return Err(ChatError::Aborted);
        }
        
        // Inject todo list message for next iteration
        if let Err(e) = inject_todo_list_message(manager, chat_id, template_loader, event_sender) {
            eprintln!("Failed to inject todo list message: {}", e);
        }
        
        // After all tools complete, check if we should pause
        if let Some(state_info) = manager.get_stream_state(chat_id).await {
            if state_info.is_paused {
                // All tool results have been inserted, exit loop
                return Ok(ToolLoopResult::Paused);
            }
        }
        
        if let Some(content) = result.content {
            *current_content = content;
        }
    }
}

pub async fn execute_tool_call<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    tool_call: &ToolCall,
    mcp_clients: &[(String, String, Arc<McpClient>)],
    event_sender: &broadcast::Sender<ChatEvent>,
) -> (ToolResult, String) {
    let tool_name = &tool_call.function.name;

    if tool_name == RHD_SET_TODO_LIST_TOOL_NAME {
        let result = handle_rhd_set_todo_list(manager, chat_id, &tool_call.function.arguments, event_sender).await;
        return (result, String::new());
    }

    if tool_name == RHD_SET_ROLE_TOOL_NAME {
        let result = handle_rhd_set_role(manager, chat_id, &tool_call.function.arguments, event_sender).await;
        return (result, String::new());
    }

    let (mcp_id, bare_tool_name) = split_tool_name(tool_name);
    for (_project_name, client_mcp_id, client) in mcp_clients {
        if *client_mcp_id == mcp_id {
            match client.call_tool(&bare_tool_name, &tool_call.function.arguments).await {
                Ok(result) => return (result, client_mcp_id.clone()),
                Err(e) => return (ToolResult {
                    content: format!("Error: {}", e),
                    is_error: Some(true),
                    raw_response: None,
                }, client_mcp_id.clone()),
            }
        }
    }
    (ToolResult {
        content: format!("Error: unknown tool '{}'", tool_call.function.name),
        is_error: Some(true),
        raw_response: None,
    }, String::new())
}
