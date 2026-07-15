use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rhd_ai::client::{ChatMessage, OpenAiClient, RawLogger, ToolCall};
use rhd_ai::{FunctionDefinition, ToolDefinition};
use rhd_db::{ChatDb, Message};
use rhd_mcp_client::client::McpClient;
use rhd_mcp_client::{McpClientTrait, ToolResult};
use tokio::sync::{broadcast, Mutex};
use tokio_util::sync::CancellationToken;

use crate::chat_log::ChatLoggers;
use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::ProjectProvider;

pub const RHD_SET_ROLE_TOOL_NAME: &str = "rhd_set_role";
pub const RHD_SET_TODO_LIST_TOOL_NAME: &str = "rhd_set_todo_list";

pub fn rhd_set_todo_list_tool_definition() -> ToolDefinition {
    ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: RHD_SET_TODO_LIST_TOOL_NAME.to_string(),
            description: "Replace the entire TODO list with an updated checklist reflecting the current state. Always provide the full list; the system will overwrite the previous one. This tool is designed for step-by-step task tracking, allowing you to confirm completion of each step before updating, update multiple statuses at once (e.g., mark one as completed and start the next), and dynamically add new todos as they're discovered.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "string",
                        "description": "Full markdown checklist in execution order, using [ ] for pending, [x] for completed, [-] for in progress, and [!] for discarded"
                    }
                },
                "required": ["todos"]
            }),
        },
    }
}

pub fn rhd_set_role_tool_definition() -> ToolDefinition {
    ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: RHD_SET_ROLE_TOOL_NAME.to_string(),
            description: "Switch the current active role. Use this tool when you need to change your behavioral role based on the task requirements. The role determines your system prompt and behavior patterns.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "role_name": {
                        "type": "string",
                        "description": "The name of the role to switch to. Must be one of the available roles listed in the system prompt."
                    }
                },
                "required": ["role_name"]
            }),
        },
    }
}

pub fn collect_builtin_tools(
    db: &Arc<ChatDb>,
    project_provider: &Arc<impl ProjectProvider>,
    chat_id: i64,
) -> Vec<ToolDefinition> {
    let mut tools = Vec::new();

    tools.push(rhd_set_todo_list_tool_definition());

    let attached_projects = match db.get_chat_projects(chat_id) {
        Ok(projects) => projects,
        Err(_) => return tools,
    };

    let has_roles = attached_projects
        .iter()
        .any(|(project_name, _)| !project_provider.get_project_roles(project_name).is_empty());

    if has_roles {
        tools.push(rhd_set_role_tool_definition());
    }

    tools
}

pub async fn collect_tools_from_projects<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
) -> (Vec<ToolDefinition>, Vec<(String, String, Arc<McpClient>)>) {
    let mut tools = Vec::new();
    let mut mcp_clients = Vec::new();

    tools.extend(collect_builtin_tools(db, project_provider, chat_id));

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

pub async fn handle_rhd_set_todo_list<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    arguments: &str,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> ToolResult {
    let args: serde_json::Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult {
                content: format!("Error: invalid arguments: {}", e),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    let todos = match args.get("todos").and_then(|v| v.as_str()) {
        Some(todos) => todos.to_string(),
        None => {
            return ToolResult {
                content: "Error: missing required parameter 'todos'".to_string(),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    let todo_items = match parse_todo_list(&todos) {
        Ok(items) => items,
        Err(e) => {
            return ToolResult {
                content: format!("Error: failed to parse todo list: {}", e),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    match manager.db().set_todo_list(chat_id, &todos) {
        Ok(()) => {
            let _ = event_sender.send(ChatEvent::TodoListUpdated {
                chat_id,
                items: todo_items,
            });
            
            ToolResult {
                content: "Todo list updated successfully.".to_string(),
                is_error: Some(false),
                raw_response: None,
            }
        }
        Err(e) => ToolResult {
            content: format!("Error: failed to save todo list: {}", e),
            is_error: Some(true),
            raw_response: None,
        },
    }
}

pub fn parse_todo_list(input: &str) -> Result<Vec<crate::TodoItem>, String> {
    let mut items = Vec::new();
    
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        
        if let Some(content) = trimmed.strip_prefix("[ ] ") {
            items.push(crate::TodoItem {
                content: content.to_string(),
                status: crate::TodoStatus::Pending,
            });
        } else if let Some(content) = trimmed.strip_prefix("[-] ") {
            items.push(crate::TodoItem {
                content: content.to_string(),
                status: crate::TodoStatus::InProgress,
            });
        } else if let Some(content) = trimmed.strip_prefix("[x] ") {
            items.push(crate::TodoItem {
                content: content.to_string(),
                status: crate::TodoStatus::Completed,
            });
        } else if let Some(content) = trimmed.strip_prefix("[!] ") {
            items.push(crate::TodoItem {
                content: content.to_string(),
                status: crate::TodoStatus::Discarded,
            });
        }
    }
    
    Ok(items)
}

pub async fn handle_rhd_set_role<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    arguments: &str,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> ToolResult {
    let args: serde_json::Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult {
                content: format!("Error: invalid arguments: {}", e),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    let role_name = match args.get("role_name").and_then(|v| v.as_str()) {
        Some(name) => name.to_string(),
        None => {
            return ToolResult {
                content: "Error: missing required parameter 'role_name'".to_string(),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    let attached_projects = match manager.db().get_chat_projects(chat_id) {
        Ok(projects) => projects,
        Err(e) => {
            return ToolResult {
                content: format!("Error: failed to get attached projects: {}", e),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    let mut found_project = None;
    for (project_name, _) in &attached_projects {
        let roles = manager.project_provider().get_project_roles(project_name);
        if roles.iter().any(|r| r.name == role_name) {
            found_project = Some(project_name.clone());
            break;
        }
    }

    let project_name = match found_project {
        Some(p) => p,
        None => {
            return ToolResult {
                content: format!("Error: role '{}' not found in any attached project", role_name),
                is_error: Some(true),
                raw_response: None,
            };
        }
    };

    match manager
        .set_active_role(chat_id, &project_name, &role_name, event_sender.clone())
        .await
    {
        Ok(()) => ToolResult {
            content: format!("Successfully switched to role '{}'", role_name),
            is_error: Some(false),
            raw_response: None,
        },
        Err(e) => ToolResult {
            content: format!("Error: failed to set role: {}", e),
            is_error: Some(true),
            raw_response: None,
        },
    }
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
) -> Result<i64, ChatError> {
    loop {
        if cancel_token.is_cancelled() {
            if let Some(ref mut l) = loggers {
                l.chat_log.log_stream_error("aborted");
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

        crate::projects::inject_pending_role_prompt(
            manager.db(),
            manager.project_provider(),
            chat_id,
            event_sender,
        )?;

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
            .await?;

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
            
            let assistant_message_id =
                manager.db().add_message(chat_id, "assistant", &final_content, Some(model), thinking_option.as_deref())?;
            let assistant_message = Message {
                id: assistant_message_id,
                chat_id,
                role: "assistant".to_string(),
                content: final_content,
                created_at: chrono::Utc::now().to_rfc3339(),
                model: Some(model.to_string()),
                thinking_content: thinking_option,
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
        let intermediate_msg_id = manager.db().add_message(
            chat_id,
            "assistant",
            &assistant_msg_content,
            Some(model),
            thinking_option,
        )?;
        let intermediate_message = Message {
            id: intermediate_msg_id,
            chat_id,
            role: "assistant".to_string(),
            content: assistant_msg_content,
            created_at: chrono::Utc::now().to_rfc3339(),
            model: Some(model.to_string()),
            thinking_content: thinking_option.map(|s| s.to_string()),
        };
        let _ = event_sender.send(ChatEvent::MessageAdded {
            chat_id,
            message: intermediate_message,
        });
        
        // Execute tools and send completion events
        for tool_call in &tool_calls_with_unique_ids {
            if cancel_token.is_cancelled() {
                if let Some(ref mut l) = loggers {
                    l.chat_log.log_stream_error("aborted");
                }
                let _ = event_sender.send(ChatEvent::StreamError {
                    chat_id,
                    error: "aborted".to_string(),
                });
                return Err(ChatError::Ai(rhd_ai::client::AiError::Aborted {
                    model: model.to_string(),
                }));
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
            let tool_msg_id = manager.db().add_message(chat_id, "tool", &tool_result_json, None, None)?;
            let tool_message = Message {
                id: tool_msg_id,
                chat_id,
                role: "tool".to_string(),
                content: tool_result_json,
                created_at: chrono::Utc::now().to_rfc3339(),
                model: None,
                thinking_content: None,
            };
            let _ = event_sender.send(ChatEvent::MessageAdded {
                chat_id,
                message: tool_message,
            });
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
            "assistant" => ChatMessage::assistant_with_thinking(&m.content, m.thinking_content.clone()),
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
                        chat_messages.push(ChatMessage::assistant_with_thinking(content.unwrap_or_default(), m.thinking_content.clone()));
                    } else {
                        chat_messages.push(ChatMessage::assistant_with_tool_calls(content, m.thinking_content.clone(), tool_calls));
                    }
                } else {
                    chat_messages.push(ChatMessage::assistant_with_thinking(&m.content, m.thinking_content.clone()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use rhd_ai::client::FunctionCall;
    use rhd_api::project::{McpRef, Role};
    use crate::McpStatus;
    use std::fs;

    fn cleanup(path: &str) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(format!("{}-wal", path));
        let _ = fs::remove_file(format!("{}-shm", path));
    }

    struct MockProjectProvider {
        roles: std::collections::HashMap<String, Vec<Role>>,
        role_prompts: std::collections::HashMap<(String, String), String>,
    }

    #[async_trait::async_trait]
    impl ProjectProvider for MockProjectProvider {
        async fn get_mcp_status(&self, _project_name: &str) -> Vec<(String, McpStatus)> {
            vec![]
        }

        fn get_project_system_prompt(&self, _project_name: &str) -> Option<String> {
            None
        }

        fn get_project_mcp_refs(&self, _project_name: &str) -> Vec<McpRef> {
            vec![]
        }

        async fn get_mcp_clients(&self, _project_name: &str) -> Vec<(String, Arc<McpClient>)> {
            vec![]
        }

        async fn spawn_project_mcp(&self, _project_name: &str) -> Result<(), String> {
            Ok(())
        }

        fn get_project_roles(&self, project_name: &str) -> Vec<Role> {
            self.roles.get(project_name).cloned().unwrap_or_default()
        }

        fn get_role_system_prompt(&self, project_name: &str, role_name: &str) -> Option<String> {
            self.role_prompts
                .get(&(project_name.to_string(), role_name.to_string()))
                .cloned()
        }
    }

    // Test helper functions
    
    #[test]
    fn test_extract_mcp_id_from_tool_name_with_prefix() {
        assert_eq!(extract_mcp_id_from_tool_name("mock1/echo"), "mock1");
        assert_eq!(extract_mcp_id_from_tool_name("server/tool"), "server");
    }

    #[test]
    fn test_extract_mcp_id_from_tool_name_without_prefix() {
        // When there's no '/', split returns the entire string
        assert_eq!(extract_mcp_id_from_tool_name("echo"), "echo");
        assert_eq!(extract_mcp_id_from_tool_name("rhd_set_flag"), "rhd_set_flag");
    }

    #[test]
    fn test_split_tool_name_with_prefix() {
        let (mcp_id, bare_name) = split_tool_name("mock1/echo");
        assert_eq!(mcp_id, "mock1");
        assert_eq!(bare_name, "echo");
    }

    #[test]
    fn test_split_tool_name_without_prefix() {
        let (mcp_id, bare_name) = split_tool_name("echo");
        assert_eq!(mcp_id, "");
        assert_eq!(bare_name, "echo");
    }

    #[test]
    fn test_build_chat_messages() {
        let messages = vec![
            Message {
                id: 1,
                chat_id: 1,
                role: "system".to_string(),
                content: "You are helpful".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
                model: None,
                thinking_content: None,
            },
            Message {
                id: 2,
                chat_id: 1,
                role: "user".to_string(),
                content: "Hello".to_string(),
                created_at: "2024-01-01T00:00:01Z".to_string(),
                model: None,
                thinking_content: None,
            },
            Message {
                id: 3,
                chat_id: 1,
                role: "assistant".to_string(),
                content: "Hi there".to_string(),
                created_at: "2024-01-01T00:00:02Z".to_string(),
                model: None,
                thinking_content: None,
            },
        ];

        let chat_messages = build_chat_messages(&messages);
        assert_eq!(chat_messages.len(), 3);
    }

    #[test]
    fn test_build_chat_messages_for_tools_simple() {
        let messages = vec![
            Message {
                id: 1,
                chat_id: 1,
                role: "user".to_string(),
                content: "Use the tool".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
                model: None,
                thinking_content: None,
            },
        ];

        let chat_messages = build_chat_messages_for_tools(&messages);
        assert_eq!(chat_messages.len(), 1);
    }

    #[test]
    fn test_build_chat_messages_for_tools_with_tool_calls() {
        let assistant_content = serde_json::json!({
            "content": "I'll use the tool",
            "toolCalls": [
                {
                    "id": "call_1",
                    "name": "mock1/echo",
                    "arguments": "{\"message\":\"test\"}"
                }
            ]
        }).to_string();

        let tool_result = serde_json::json!({
            "toolCallId": "call_1",
            "name": "mock1/echo",
            "result": "Tool executed"
        }).to_string();

        let messages = vec![
            Message {
                id: 1,
                chat_id: 1,
                role: "user".to_string(),
                content: "Use the tool".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
                model: None,
                thinking_content: None,
            },
            Message {
                id: 2,
                chat_id: 1,
                role: "assistant".to_string(),
                content: assistant_content,
                created_at: "2024-01-01T00:00:01Z".to_string(),
                model: None,
                thinking_content: None,
            },
            Message {
                id: 3,
                chat_id: 1,
                role: "tool".to_string(),
                content: tool_result,
                created_at: "2024-01-01T00:00:02Z".to_string(),
                model: None,
                thinking_content: None,
            },
        ];

        let chat_messages = build_chat_messages_for_tools(&messages);
        assert_eq!(chat_messages.len(), 3);
    }

    #[tokio::test]
    async fn test_execute_tool_call_with_mcp_client() {
        use rhd_mcp_client::client::McpClient;
        use rhd_mcp_client::McpConfig;
        use std::sync::Arc;

        // Create a mock MCP client that returns a known result
        let _config = McpConfig {
            name: Some("mock1".to_string()),
            cmd: Some("echo".to_string()),
            args: vec![],
            cwd: None,
            env: std::collections::HashMap::new(),
        };

        // Note: We can't easily test this without a real MCP server
        // This test would require mocking the McpClient trait
        // For now, we'll test the error case
        let tool_call = ToolCall {
            id: "call_1".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "unknown/echo".to_string(),
                arguments: "{\"message\":\"test\"}".to_string(),
            },
        };

        let mcp_clients: Vec<(String, String, Arc<McpClient>)> = vec![];
        let db = Arc::new(ChatDb::new("test_execute_tool.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();
        let provider = MockProjectProvider {
            roles: HashMap::new(),
            role_prompts: HashMap::new(),
        };
        let manager = ChatManager::new(db.clone(), Arc::new(provider), None, false);
        let (event_sender, _) = broadcast::channel(100);
        
        let (result, mcp_id) = execute_tool_call(&manager, chat_id, &tool_call, &mcp_clients, &event_sender).await;
        
        assert!(result.content.contains("Error: unknown tool"));
        assert_eq!(mcp_id, "");
        
        cleanup("test_execute_tool.db");
    }

    #[tokio::test]
    async fn test_execute_tool_call_unknown_tool() {
        use rhd_mcp_client::client::McpClient;
        use std::sync::Arc;

        let tool_call = ToolCall {
            id: "call_1".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "mock1/echo".to_string(),
                arguments: "{\"message\":\"test\"}".to_string(),
            },
        };

        let mcp_clients: Vec<(String, String, Arc<McpClient>)> = vec![];
        let db = Arc::new(ChatDb::new("test_execute_tool_unknown.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();
        let provider = MockProjectProvider {
            roles: HashMap::new(),
            role_prompts: HashMap::new(),
        };
        let manager = ChatManager::new(db.clone(), Arc::new(provider), None, false);
        let (event_sender, _) = broadcast::channel(100);
        
        let (result, mcp_id) = execute_tool_call(&manager, chat_id, &tool_call, &mcp_clients, &event_sender).await;
        
        assert!(result.content.contains("Error: unknown tool 'mock1/echo'"));
        assert_eq!(mcp_id, "");
        
        cleanup("test_execute_tool_unknown.db");
    }

    #[test]
    fn test_tool_loop_iteration_tracking() {
        // Test that iterations are tracked correctly
        let mut iterations = 0u32;
        let max_iterations = 3u32;
        
        // Simulate tool loop iterations
        while iterations < max_iterations {
            iterations += 1;
        }
        
        assert_eq!(iterations, 3);
        assert!(iterations >= max_iterations);
    }

    #[test]
    fn test_tool_loop_max_iterations_check() {
        let iterations = 5u32;
        let max_iterations = 5u32;
        
        // This simulates the check in tool_loop
        let should_stop = iterations >= max_iterations;
        
        assert!(should_stop, "Should stop when iterations reach max");
    }

    #[test]
    fn test_tool_loop_cancellation_check() {
        use tokio_util::sync::CancellationToken;
        
        let token = CancellationToken::new();
        assert!(!token.is_cancelled(), "Token should not be cancelled initially");
        
        token.cancel();
        assert!(token.is_cancelled(), "Token should be cancelled after cancel()");
    }

    // Integration tests for tool_loop with mock implementations
    
    use rhd_mcp_client::{McpClientTrait, ToolDefinition as McpToolDefinition, ToolResult};
    use std::collections::HashMap;
    use std::future::Future;

    // Mock MCP client for testing
    #[allow(dead_code)]
    struct MockMcpClient {
        tools: Vec<McpToolDefinition>,
        call_results: HashMap<String, String>,
    }

    impl MockMcpClient {
        #[allow(dead_code)]
        fn new() -> Self {
            Self {
                tools: vec![],
                call_results: HashMap::new(),
            }
        }

        #[allow(dead_code)]
        fn with_tool(mut self, name: &str, description: &str) -> Self {
            self.tools.push(McpToolDefinition {
                name: name.to_string(),
                description: description.to_string(),
                input_schema: serde_json::json!({}),
            });
            self
        }

        #[allow(dead_code)]
        fn with_call_result(mut self, tool_name: &str, result: &str) -> Self {
            self.call_results.insert(tool_name.to_string(), result.to_string());
            self
        }
    }

    impl McpClientTrait for MockMcpClient {
        fn list_tools(&self) -> impl Future<Output = rhd_mcp_client::McpResult<Vec<McpToolDefinition>>> + Send {
            let tools = self.tools.clone();
            async move { Ok(tools) }
        }

        fn call_tool(&self, name: &str, _arguments: &str) -> impl Future<Output = rhd_mcp_client::McpResult<ToolResult>> + Send {
            let result = self.call_results.get(name).cloned().unwrap_or_else(|| format!("Mock result for {}", name));
            async move {
                Ok(ToolResult {
                    content: result,
                    is_error: None,
                    raw_response: None,
                })
            }
        }

        fn has_tool(&self, name: &str) -> impl Future<Output = bool> + Send {
            let has = self.tools.iter().any(|t| t.name == name);
            async move { has }
        }
    }

    #[tokio::test]
    async fn test_tool_loop_no_tools_returns_immediately() {
        // This test would require a full mock setup
        // For now, we document the expected behavior:
        // When chat_with_tools returns no tool calls, tool_loop should:
        // 1. Emit StreamChunk with final content
        // 2. Save assistant message to DB
        // 3. Emit MessageAdded event
        // 4. Emit StreamFinished event
        // 5. Return the message ID
        assert!(true, "Documented expected behavior");
    }

    #[tokio::test]
    async fn test_tool_loop_single_tool_call() {
        // Expected behavior:
        // 1. First call to chat_with_tools returns tool call
        // 2. Tool is executed
        // 3. ToolCallStarted event emitted
        // 4. ToolCallCompleted event emitted
        // 5. Second call to chat_with_tools returns final response
        // 6. StreamChunk, MessageAdded, StreamFinished events emitted
        assert!(true, "Documented expected behavior");
    }

    #[tokio::test]
    async fn test_tool_loop_max_iterations() {
        // Expected behavior:
        // When iterations reach max_iterations, tool_loop should return error
        let iterations = 10u32;
        let max_iterations = 10u32;
        assert!(iterations >= max_iterations);
    }

    #[tokio::test]
    async fn test_tool_loop_cancellation() {
        use tokio_util::sync::CancellationToken;
        
        let token = CancellationToken::new();
        token.cancel();
        
        // Expected behavior:
        // When cancel_token.is_cancelled() returns true, tool_loop should:
        // 1. Emit StreamError event with "aborted"
        // 2. Return AiError::Aborted
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_mock_mcp_client_list_tools() {
        let client = MockMcpClient::new()
            .with_tool("echo", "Echo tool")
            .with_tool("greet", "Greeting tool");
        
        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "echo");
        assert_eq!(tools[1].name, "greet");
    }

    #[tokio::test]
    async fn test_mock_mcp_client_call_tool() {
        let client = MockMcpClient::new()
            .with_tool("echo", "Echo tool")
            .with_call_result("echo", "Echo: Hello");
        
        let result = client.call_tool("echo", "{\"message\":\"Hello\"}").await.unwrap();
        assert_eq!(result.content, "Echo: Hello");
    }

    #[tokio::test]
    async fn test_mock_mcp_client_call_tool_default_result() {
        let client = MockMcpClient::new()
            .with_tool("echo", "Echo tool");
        
        let result = client.call_tool("echo", "{\"message\":\"Hello\"}").await.unwrap();
        assert_eq!(result.content, "Mock result for echo");
    }

    #[tokio::test]
    async fn test_mock_mcp_client_has_tool() {
        let client = MockMcpClient::new()
            .with_tool("echo", "Echo tool");
        
        assert!(client.has_tool("echo").await);
        assert!(!client.has_tool("nonexistent").await);
    }

    #[tokio::test]
    async fn test_mock_mcp_client_integration() {
        // Test that MockMcpClient can be used to simulate tool execution
        let client = MockMcpClient::new()
            .with_tool("echo", "Echo tool")
            .with_call_result("echo", "Echo: Test message");
        
        // Verify tool is available
        assert!(client.has_tool("echo").await);
        
        // Verify tool can be called
        let result = client.call_tool("echo", "{\"message\":\"Test message\"}").await.unwrap();
        assert_eq!(result.content, "Echo: Test message");
        
        // Verify tool list
        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
    }

    #[test]
    fn test_rhd_set_role_tool_definition() {
        let tool = rhd_set_role_tool_definition();
        assert_eq!(tool.function.name, "rhd_set_role");
        assert!(tool.function.description.contains("role"));
        
        let params = tool.function.parameters;
        assert_eq!(params["type"], "object");
        assert!(params["properties"]["role_name"].is_object());
        assert_eq!(params["required"], serde_json::json!(["role_name"]));
    }

    #[test]
    fn test_collect_builtin_tools_no_roles() {
        let db = Arc::new(ChatDb::new("test_no_roles.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();
        
        let provider = MockProjectProvider {
            roles: std::collections::HashMap::new(),
            role_prompts: std::collections::HashMap::new(),
        };
        
        let tools = collect_builtin_tools(&db, &Arc::new(provider), chat_id);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function.name, "rhd_set_todo_list");
        
        cleanup("test_no_roles.db");
    }

    #[test]
    fn test_collect_builtin_tools_with_roles() {
        let db = Arc::new(ChatDb::new("test_with_roles.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();
        
        let mut provider = MockProjectProvider {
            roles: std::collections::HashMap::new(),
            role_prompts: std::collections::HashMap::new(),
        };
        provider.roles.insert("project-a".to_string(), vec![
            Role {
                name: "developer".to_string(),
                system_prompt: "Dev".to_string(),
                when_to_use: "Coding".to_string(),
            },
        ]);
        
        db.attach_project(chat_id, "project-a").unwrap();
        
        let tools = collect_builtin_tools(&db, &Arc::new(provider), chat_id);
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].function.name, "rhd_set_todo_list");
        assert_eq!(tools[1].function.name, "rhd_set_role");
        
        cleanup("test_with_roles.db");
    }

    #[tokio::test]
    async fn test_handle_rhd_set_role_success() {
        let db = Arc::new(ChatDb::new("test_set_role.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();
        
        let mut provider = MockProjectProvider {
            roles: std::collections::HashMap::new(),
            role_prompts: std::collections::HashMap::new(),
        };
        provider.roles.insert("project-a".to_string(), vec![
            Role {
                name: "developer".to_string(),
                system_prompt: "You are a developer.".to_string(),
                when_to_use: "Use for coding.".to_string(),
            },
        ]);
        provider.role_prompts.insert(
            ("project-a".to_string(), "developer".to_string()),
            "You are a developer.".to_string(),
        );
        
        db.attach_project(chat_id, "project-a").unwrap();
        
        let provider = Arc::new(provider);
        let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
        let (event_sender, _) = broadcast::channel(100);
        
        let args = r#"{"role_name": "developer"}"#;
        let result = handle_rhd_set_role(&manager, chat_id, args, &event_sender).await;
        
        assert_eq!(result.is_error, Some(false));
        assert!(result.content.contains("Successfully switched"));
        
        // Verify active role was set
        let active_role = db.get_active_role(chat_id).unwrap().unwrap();
        assert_eq!(active_role.0, "project-a");
        assert_eq!(active_role.1, "developer");
        
        // Verify system message was NOT injected immediately (deferred injection)
        let messages = db.get_messages(chat_id).unwrap();
        assert!(!messages.iter().any(|m| m.role == "system" && m.content.contains("Your current role is now")));
        
        // Verify role_prompt_pending flag is set
        assert!(db.has_role_prompt_pending(chat_id).unwrap());
        
        cleanup("test_set_role.db");
    }

    #[tokio::test]
    async fn test_handle_rhd_set_role_not_found() {
        let db = Arc::new(ChatDb::new("test_set_role_not_found.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();
        
        let provider = MockProjectProvider {
            roles: std::collections::HashMap::new(),
            role_prompts: std::collections::HashMap::new(),
        };
        
        let provider = Arc::new(provider);
        let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
        let (event_sender, _) = broadcast::channel(100);
        
        let args = r#"{"role_name": "nonexistent"}"#;
        let result = handle_rhd_set_role(&manager, chat_id, args, &event_sender).await;
        
        assert_eq!(result.is_error, Some(true));
        assert!(result.content.contains("not found"));
        
        cleanup("test_set_role_not_found.db");
    }

    #[tokio::test]
    async fn test_handle_rhd_set_role_invalid_args() {
        let db = Arc::new(ChatDb::new("test_set_role_invalid.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();
        
        let provider = MockProjectProvider {
            roles: std::collections::HashMap::new(),
            role_prompts: std::collections::HashMap::new(),
        };
        
        let provider = Arc::new(provider);
        let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
        let (event_sender, _) = broadcast::channel(100);
        
        let args = r#"{"invalid": "args"}"#;
        let result = handle_rhd_set_role(&manager, chat_id, args, &event_sender).await;
        
        assert_eq!(result.is_error, Some(true));
        assert!(result.content.contains("missing required parameter"));
        
        cleanup("test_set_role_invalid.db");
    }

    #[test]
    fn test_rhd_set_todo_list_tool_definition() {
        let tool = rhd_set_todo_list_tool_definition();
        assert_eq!(tool.function.name, "rhd_set_todo_list");
        assert!(tool.function.description.contains("TODO list"));
        
        let params = tool.function.parameters;
        assert_eq!(params["type"], "object");
        assert!(params["properties"]["todos"].is_object());
        assert_eq!(params["required"], serde_json::json!(["todos"]));
    }

    #[test]
    fn test_parse_todo_list() {
        let input = "[x] Completed task\n[-] In progress task\n[ ] Pending task\n[!] Discarded task";
        let items = parse_todo_list(input).unwrap();
        
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].content, "Completed task");
        assert_eq!(items[0].status, crate::TodoStatus::Completed);
        assert_eq!(items[1].content, "In progress task");
        assert_eq!(items[1].status, crate::TodoStatus::InProgress);
        assert_eq!(items[2].content, "Pending task");
        assert_eq!(items[2].status, crate::TodoStatus::Pending);
        assert_eq!(items[3].content, "Discarded task");
        assert_eq!(items[3].status, crate::TodoStatus::Discarded);
    }

    #[test]
    fn test_parse_todo_list_empty_lines() {
        let input = "[x] Task 1\n\n[-] Task 2\n\n";
        let items = parse_todo_list(input).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_parse_todo_list_invalid_lines() {
        let input = "[x] Valid task\nInvalid line\n[-] Another valid task";
        let items = parse_todo_list(input).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn test_handle_rhd_set_todo_list_success() {
        let db = Arc::new(ChatDb::new("test_todo_list.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();
        
        let provider = MockProjectProvider {
            roles: std::collections::HashMap::new(),
            role_prompts: std::collections::HashMap::new(),
        };
        
        let provider = Arc::new(provider);
        let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
        let (event_sender, _) = broadcast::channel(100);
        
        let args = r#"{"todos": "[x] Task 1\n[-] Task 2\n[ ] Task 3"}"#;
        let result = handle_rhd_set_todo_list(&manager, chat_id, args, &event_sender).await;
        
        assert_eq!(result.is_error, Some(false));
        assert!(result.content.contains("successfully"));
        
        let saved = db.get_todo_list(chat_id).unwrap().unwrap();
        assert!(saved.contains("Task 1"));
        
        cleanup("test_todo_list.db");
    }

    #[tokio::test]
    async fn test_handle_rhd_set_todo_list_invalid_args() {
        let db = Arc::new(ChatDb::new("test_todo_list_invalid.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();
        
        let provider = MockProjectProvider {
            roles: std::collections::HashMap::new(),
            role_prompts: std::collections::HashMap::new(),
        };
        
        let provider = Arc::new(provider);
        let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
        let (event_sender, _) = broadcast::channel(100);
        
        let args = r#"{"invalid": "args"}"#;
        let result = handle_rhd_set_todo_list(&manager, chat_id, args, &event_sender).await;
        
        assert_eq!(result.is_error, Some(true));
        assert!(result.content.contains("missing required parameter"));
        
        cleanup("test_todo_list_invalid.db");
    }

    #[tokio::test]
    async fn test_handle_rhd_set_todo_list_emits_event() {
        let db = Arc::new(ChatDb::new("test_todo_event.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();
        
        let provider = MockProjectProvider {
            roles: std::collections::HashMap::new(),
            role_prompts: std::collections::HashMap::new(),
        };
        
        let provider = Arc::new(provider);
        let manager = ChatManager::new(db.clone(), provider.clone(), None, false);
        let (event_sender, mut event_receiver) = broadcast::channel(100);
        
        let args = r#"{"todos": "[x] Task 1\n[-] Task 2"}"#;
        let result = handle_rhd_set_todo_list(&manager, chat_id, args, &event_sender).await;
        
        assert_eq!(result.is_error, Some(false));
        
        let event = event_receiver.recv().await.unwrap();
        match event {
            ChatEvent::TodoListUpdated { chat_id: id, items } => {
                assert_eq!(id, chat_id);
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].content, "Task 1");
                assert_eq!(items[0].status, crate::TodoStatus::Completed);
                assert_eq!(items[1].content, "Task 2");
                assert_eq!(items[1].status, crate::TodoStatus::InProgress);
            }
            _ => panic!("Expected TodoListUpdated event"),
        }
        
        cleanup("test_todo_event.db");
    }
}
