use std::collections::HashMap;
use std::sync::Arc;

use rhd_ai::client::{ChatMessage, OpenAiClient, ToolCall};
use rhd_ai::config::ModelConfig;
use rhd_ai::{FunctionDefinition, ToolDefinition};
use rhd_api::project::ProjectInfo;
use rhd_db::{ChatDb, ChatInfo, Message};
use rhd_mcp_client::McpClientTrait;
use thiserror::Error;
use tokio::sync::{broadcast, Mutex, Notify};
use tokio_util::sync::CancellationToken;

use crate::mcp_cache::McpServerCache;
use crate::project_manager::ProjectManager;

#[derive(Debug)]
enum StreamState {
    Running {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
    },
    Paused {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
    },
}

impl StreamState {
    fn cancel_token(&self) -> &CancellationToken {
        match self {
            StreamState::Running { cancel_token, .. } => cancel_token,
            StreamState::Paused { cancel_token, .. } => cancel_token,
        }
    }
}

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("database error: {0}")]
    Db(#[from] rhd_db::DbError),

    #[error("AI error: {0}")]
    Ai(#[from] rhd_ai::client::AiError),

    #[error("chat not found")]
    ChatNotFound,

    #[error("message not found")]
    MessageNotFound,

    #[error("model not found: {0}")]
    ModelNotFound(String),

    #[error("project not found: {0}")]
    ProjectNotFound(String),

    #[error("MCP server not connected for project: {0}")]
    McpNotConnected(String),
}

#[derive(Debug, Clone)]
pub enum ChatEvent {
    StreamChunk { chat_id: i64, content: String },
    StreamFinished { chat_id: i64, message_id: i64, finish_reason: String },
    StreamError { chat_id: i64, error: String },
    MessageAdded { chat_id: i64, message: Message },
    DevNotification { title: String, message: String },
    ProjectAttached { chat_id: i64, project_name: String },
    ProjectDetached { chat_id: i64, project_name: String },
    ToolCallStarted {
        chat_id: i64,
        tool_call_id: String,
        tool_name: String,
        arguments: String,
    },
    ToolCallCompleted {
        chat_id: i64,
        tool_call_id: String,
        result: String,
    },
    ChatPaused { chat_id: i64 },
    ChatResumed { chat_id: i64 },
}

pub struct ChatManager {
    db: Arc<ChatDb>,
    active_streams: Mutex<HashMap<i64, StreamState>>,
}

impl ChatManager {
    pub fn new(db: Arc<ChatDb>) -> Self {
        Self {
            db,
            active_streams: Mutex::new(HashMap::new()),
        }
    }

    pub fn create_chat(&self, title: &str) -> Result<i64, ChatError> {
        Ok(self.db.create_chat(title)?)
    }

    pub fn list_chats(&self) -> Result<Vec<ChatInfo>, ChatError> {
        Ok(self.db.list_chats()?)
    }

    pub fn get_chat(&self, id: i64) -> Result<Option<(ChatInfo, Vec<Message>)>, ChatError> {
        let chat = self.db.get_chat(id)?;
        match chat {
            Some(chat_info) => {
                let messages = self.db.get_messages(id)?;
                Ok(Some((chat_info, messages)))
            }
            None => Ok(None),
        }
    }

    pub fn delete_chat(&self, id: i64) -> Result<(), ChatError> {
        self.db.delete_chat(id)?;
        Ok(())
    }

    pub async fn send_message(
        &self,
        chat_id: i64,
        content: String,
        model: &str,
        models: &HashMap<String, ModelConfig>,
        project_manager: &ProjectManager,
        mcp_cache: &McpServerCache,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<i64, ChatError> {
        if self.db.get_chat(chat_id)?.is_none() {
            return Err(ChatError::ChatNotFound);
        }

        // Check if chat is paused - if so, add message and resume tool loop
        let pause_notify = {
            let active = self.active_streams.lock().await;
            if let Some(StreamState::Paused { pause_notify, .. }) = active.get(&chat_id) {
                Some(pause_notify.clone())
            } else {
                None
            }
        };

        if let Some(notify) = pause_notify {
            let user_message_id = self.db.add_message(chat_id, "user", &content, Some(model))?;
            let user_message = Message {
                id: user_message_id,
                chat_id,
                role: "user".to_string(),
                content,
                created_at: chrono::Utc::now().to_rfc3339(),
                model: Some(model.to_string()),
            };
            let _ = event_sender.send(ChatEvent::MessageAdded {
                chat_id,
                message: user_message,
            });

            notify.notify_one();

            return Ok(user_message_id);
        }

        let model_config = models.get(model).ok_or_else(|| ChatError::ModelNotFound(model.to_string()))?;

        // Check attached projects and inject system prompts
        let attached_projects = self.db.get_chat_projects(chat_id)?;
        for (project_name, system_prompt_added) in &attached_projects {
            // Check MCP status
            let mcp_status = project_manager.get_mcp_status(project_name).await;
            for (mcp_name, status) in &mcp_status {
                if !matches!(status, crate::project_manager::McpStatus::Connected) {
                    return Err(ChatError::McpNotConnected(format!(
                        "{}:{}",
                        project_name, mcp_name
                    )));
                }
            }

            // Inject system prompt if not already added
            if !system_prompt_added {
                if let Some(project) = project_manager.get_project(project_name) {
                    if let Some(ref system_prompt) = project.system_prompt {
                        self.db.add_message(chat_id, "system", system_prompt, None)?;
                        self.db.mark_system_prompt_added(chat_id, project_name)?;
                    }
                }
            }
        }

        // Collect tools from attached projects
        let (tools, mcp_clients) = self.collect_tools_from_projects(chat_id, project_manager).await;

        // Route to tool execution loop if tools are available
        if !tools.is_empty() {
            return self
                .send_message_with_tools(
                    chat_id,
                    content,
                    model,
                    model_config,
                    tools,
                    mcp_clients,
                    event_sender,
                )
                .await;
        }

        // Update chat's active model
        self.db.update_chat_active_model(chat_id, model)?;

        let user_message_id = self.db.add_message(chat_id, "user", &content, Some(model))?;
        let user_message = Message {
            id: user_message_id,
            chat_id,
            role: "user".to_string(),
            content,
            created_at: chrono::Utc::now().to_rfc3339(),
            model: Some(model.to_string()),
        };
        let _ = event_sender.send(ChatEvent::MessageAdded {
            chat_id,
            message: user_message,
        });

        let messages = self.db.get_messages(chat_id)?;
        let chat_messages: Vec<ChatMessage> = messages
            .iter()
            .map(|m| match m.role.as_str() {
                "user" => ChatMessage::user(&m.content),
                "assistant" => ChatMessage::assistant(&m.content),
                "system" => ChatMessage::system(&m.content),
                _ => ChatMessage::user(&m.content),
            })
            .collect();

        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());
        {
            let mut active = self.active_streams.lock().await;
            if let Some(existing) = active.get(&chat_id) {
                existing.cancel_token().cancel();
            }
            active.insert(
                chat_id,
                StreamState::Running {
                    cancel_token: cancel_token.clone(),
                    pause_notify: pause_notify.clone(),
                },
            );
        }

        let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);
        let accumulated_content = Arc::new(Mutex::new(String::new()));
        let accumulated_clone = accumulated_content.clone();
        let sender_for_closure = event_sender.clone();

        let api_model = &model_config.model;
        let result = client
            .chat_stream_cancellable(api_model, &chat_messages, cancel_token.clone(), move |chunk| {
                let sender = sender_for_closure.clone();
                let acc = accumulated_clone.clone();
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
                })
            })
            .await;

        {
            let mut active = self.active_streams.lock().await;
            active.remove(&chat_id);
        }

        match result {
            Ok(stream_result) => {
                let full_content = accumulated_content.lock().await.clone();
                let assistant_message_id = self.db.add_message(chat_id, "assistant", &full_content, Some(model))?;
                let assistant_message = Message {
                    id: assistant_message_id,
                    chat_id,
                    role: "assistant".to_string(),
                    content: full_content,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    model: Some(model.to_string()),
                };
                let _ = event_sender.send(ChatEvent::MessageAdded {
                    chat_id,
                    message: assistant_message,
                });
                let finish_reason = stream_result.finish_reason.unwrap_or_else(|| "stop".to_string());
                let _ = event_sender.send(ChatEvent::StreamFinished {
                    chat_id,
                    message_id: assistant_message_id,
                    finish_reason,
                });
                Ok(assistant_message_id)
            }
            Err(rhd_ai::client::AiError::Aborted { .. }) => {
                let _ = event_sender.send(ChatEvent::StreamError {
                    chat_id,
                    error: "aborted".to_string(),
                });
                Err(ChatError::Ai(rhd_ai::client::AiError::Aborted {
                    model: model.to_string(),
                }))
            }
            Err(err) => {
                let _ = event_sender.send(ChatEvent::StreamError {
                    chat_id,
                    error: err.to_string(),
                });
                Err(ChatError::Ai(err))
            }
        }
    }

    async fn collect_tools_from_projects(
        &self,
        chat_id: i64,
        project_manager: &ProjectManager,
    ) -> (Vec<ToolDefinition>, Vec<(String, Arc<rhd_mcp_client::client::McpClient>)>) {
        let mut tools = Vec::new();
        let mut mcp_clients = Vec::new();

        let attached_projects = match self.db.get_chat_projects(chat_id) {
            Ok(projects) => projects,
            Err(_) => return (tools, mcp_clients),
        };

        for (project_name, _) in attached_projects {
            let clients = project_manager.get_mcp_clients(&project_name).await;
            for client in clients {
                if let Ok(client_tools) = client.list_tools().await {
                    for tool in client_tools {
                        tools.push(ToolDefinition {
                            tool_type: "function".to_string(),
                            function: FunctionDefinition {
                                name: tool.name.clone(),
                                description: tool.description,
                                parameters: tool.input_schema,
                            },
                        });
                    }
                    mcp_clients.push((project_name.clone(), client));
                }
            }
        }

        (tools, mcp_clients)
    }

    async fn send_message_with_tools(
        &self,
        chat_id: i64,
        content: String,
        model: &str,
        model_config: &ModelConfig,
        tools: Vec<ToolDefinition>,
        mcp_clients: Vec<(String, Arc<rhd_mcp_client::client::McpClient>)>,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<i64, ChatError> {
        const MAX_ITERATIONS: u32 = 20;

        // Update chat's active model
        self.db.update_chat_active_model(chat_id, model)?;

        let user_message_id = self.db.add_message(chat_id, "user", &content, Some(model))?;
        let user_message = Message {
            id: user_message_id,
            chat_id,
            role: "user".to_string(),
            content: content.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            model: Some(model.to_string()),
        };
        let _ = event_sender.send(ChatEvent::MessageAdded {
            chat_id,
            message: user_message,
        });

        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());
        {
            let mut active = self.active_streams.lock().await;
            if let Some(existing) = active.get(&chat_id) {
                existing.cancel_token().cancel();
            }
            active.insert(
                chat_id,
                StreamState::Running {
                    cancel_token: cancel_token.clone(),
                    pause_notify: pause_notify.clone(),
                },
            );
        }

        let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);
        let mut iterations = 0u32;
        let mut current_content = content;

        let result = self
            .tool_loop(
                chat_id,
                model,
                &model_config.model,
                &client,
                &tools,
                &mcp_clients,
                &cancel_token,
                &event_sender,
                &mut iterations,
                &mut current_content,
                MAX_ITERATIONS,
            )
            .await;

        {
            let mut active = self.active_streams.lock().await;
            active.remove(&chat_id);
        }

        result
    }

    async fn tool_loop(
        &self,
        chat_id: i64,
        model: &str,
        api_model: &str,
        client: &OpenAiClient,
        tools: &[ToolDefinition],
        mcp_clients: &[(String, Arc<rhd_mcp_client::client::McpClient>)],
        cancel_token: &CancellationToken,
        event_sender: &broadcast::Sender<ChatEvent>,
        iterations: &mut u32,
        current_content: &mut String,
        max_iterations: u32,
    ) -> Result<i64, ChatError> {
        loop {
            if cancel_token.is_cancelled() {
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

            // Check if paused before each iteration
            self.check_pause_state(chat_id, event_sender).await;

            let messages = self.db.get_messages(chat_id)?;
            let chat_messages: Vec<ChatMessage> = messages
                .iter()
                .map(|m| match m.role.as_str() {
                    "user" => ChatMessage::user(&m.content),
                    "assistant" => ChatMessage::assistant(&m.content),
                    "system" => ChatMessage::system(&m.content),
                    "tool" => {
                        if let Ok(tool_data) = serde_json::from_str::<serde_json::Value>(&m.content) {
                            if let Some(tool_call_id) = tool_data.get("toolCallId").and_then(|v| v.as_str()) {
                                ChatMessage::tool(tool_call_id, tool_data.get("result").and_then(|v| v.as_str()).unwrap_or(""))
                            } else {
                                ChatMessage::user(&m.content)
                            }
                        } else {
                            ChatMessage::user(&m.content)
                        }
                    }
                    _ => ChatMessage::user(&m.content),
                })
                .collect();

            let system_prompts: Vec<&str> = messages
                .iter()
                .filter(|m| m.role == "system")
                .map(|m| m.content.as_str())
                .collect();

            let result = client
                .chat_with_tools(api_model, &system_prompts, current_content, tools, &[])
                .await?;

            if result.tool_calls.is_empty() {
                let final_content = result.content.unwrap_or_default();
                let assistant_message_id =
                    self.db.add_message(chat_id, "assistant", &final_content, Some(model))?;
                let assistant_message = Message {
                    id: assistant_message_id,
                    chat_id,
                    role: "assistant".to_string(),
                    content: final_content,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    model: Some(model.to_string()),
                };
                let _ = event_sender.send(ChatEvent::MessageAdded {
                    chat_id,
                    message: assistant_message,
                });
                let finish_reason = result.finish_reason.unwrap_or_else(|| "stop".to_string());
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
            self.db
                .add_message(chat_id, "assistant", &assistant_msg_content, Some(model))?;

            for tool_call in &result.tool_calls {
                if cancel_token.is_cancelled() {
                    let _ = event_sender.send(ChatEvent::StreamError {
                        chat_id,
                        error: "aborted".to_string(),
                    });
                    return Err(ChatError::Ai(rhd_ai::client::AiError::Aborted {
                        model: model.to_string(),
                    }));
                }

                let _ = event_sender.send(ChatEvent::ToolCallStarted {
                    chat_id,
                    tool_call_id: tool_call.id.clone(),
                    tool_name: tool_call.name.clone(),
                    arguments: tool_call.arguments.clone(),
                });

                let tool_result = self.execute_tool_call(tool_call, mcp_clients).await;

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
                self.db
                    .add_message(chat_id, "tool", &tool_result_json, None)?;
            }

            if let Some(content) = result.content {
                *current_content = content;
            }
        }
    }

    async fn check_pause_state(
        &self,
        chat_id: i64,
        event_sender: &broadcast::Sender<ChatEvent>,
    ) {
        let pause_notify = {
            let active = self.active_streams.lock().await;
            if let Some(StreamState::Paused { pause_notify, .. }) = active.get(&chat_id) {
                Some(pause_notify.clone())
            } else {
                None
            }
        };

        if let Some(notify) = pause_notify {
            let _ = event_sender.send(ChatEvent::ChatPaused { chat_id });
            notify.notified().await;
            let _ = event_sender.send(ChatEvent::ChatResumed { chat_id });
        }
    }

    async fn execute_tool_call(
        &self,
        tool_call: &ToolCall,
        mcp_clients: &[(String, Arc<rhd_mcp_client::client::McpClient>)],
    ) -> String {
        for (_project_name, client) in mcp_clients {
            if client.has_tool(&tool_call.name).await {
                match client.call_tool(&tool_call.name, &tool_call.arguments).await {
                    Ok(result) => return result.content,
                    Err(e) => return format!("Error: {}", e),
                }
            }
        }
        format!("Error: unknown tool '{}'", tool_call.name)
    }

    pub async fn edit_and_resend(
        &self,
        message_id: i64,
        new_content: String,
        model: &str,
        models: &HashMap<String, ModelConfig>,
        project_manager: &ProjectManager,
        mcp_cache: &McpServerCache,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<i64, ChatError> {
        let original_message = self.db.get_message(message_id)?.ok_or(ChatError::MessageNotFound)?;
        let chat_id = original_message.chat_id;

        self.db.update_message(message_id, &new_content)?;
        self.db.truncate_messages(chat_id, message_id)?;

        // Check attached projects and inject system prompts
        let attached_projects = self.db.get_chat_projects(chat_id)?;
        for (project_name, system_prompt_added) in &attached_projects {
            // Check MCP status
            let mcp_status = project_manager.get_mcp_status(project_name).await;
            for (mcp_name, status) in &mcp_status {
                if !matches!(status, crate::project_manager::McpStatus::Connected) {
                    return Err(ChatError::McpNotConnected(format!(
                        "{}:{}",
                        project_name, mcp_name
                    )));
                }
            }

            // Inject system prompt if not already added
            if !system_prompt_added {
                if let Some(project) = project_manager.get_project(project_name) {
                    if let Some(ref system_prompt) = project.system_prompt {
                        self.db.add_message(chat_id, "system", system_prompt, None)?;
                        self.db.mark_system_prompt_added(chat_id, project_name)?;
                    }
                }
            }
        }

        // Update chat's active model
        self.db.update_chat_active_model(chat_id, model)?;

        let updated_message = Message {
            id: message_id,
            chat_id,
            role: original_message.role.clone(),
            content: new_content,
            created_at: chrono::Utc::now().to_rfc3339(),
            model: Some(model.to_string()),
        };
        let _ = event_sender.send(ChatEvent::MessageAdded {
            chat_id,
            message: updated_message,
        });

        let messages = self.db.get_messages(chat_id)?;
        let chat_messages: Vec<ChatMessage> = messages
            .iter()
            .map(|m| match m.role.as_str() {
                "user" => ChatMessage::user(&m.content),
                "assistant" => ChatMessage::assistant(&m.content),
                "system" => ChatMessage::system(&m.content),
                _ => ChatMessage::user(&m.content),
            })
            .collect();

        let model_config = models.get(model).ok_or_else(|| ChatError::ModelNotFound(model.to_string()))?;

        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());
        {
            let mut active = self.active_streams.lock().await;
            if let Some(existing) = active.get(&chat_id) {
                existing.cancel_token().cancel();
            }
            active.insert(
                chat_id,
                StreamState::Running {
                    cancel_token: cancel_token.clone(),
                    pause_notify: pause_notify.clone(),
                },
            );
        }

        let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);
        let accumulated_content = Arc::new(Mutex::new(String::new()));
        let accumulated_clone = accumulated_content.clone();
        let sender_for_closure = event_sender.clone();

        let api_model = &model_config.model;
        let result = client
            .chat_stream_cancellable(api_model, &chat_messages, cancel_token.clone(), move |chunk| {
                let sender = sender_for_closure.clone();
                let acc = accumulated_clone.clone();
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
                })
            })
            .await;

        {
            let mut active = self.active_streams.lock().await;
            active.remove(&chat_id);
        }

        match result {
            Ok(stream_result) => {
                let full_content = accumulated_content.lock().await.clone();
                let assistant_message_id = self.db.add_message(chat_id, "assistant", &full_content, Some(model))?;
                let assistant_message = Message {
                    id: assistant_message_id,
                    chat_id,
                    role: "assistant".to_string(),
                    content: full_content,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    model: Some(model.to_string()),
                };
                let _ = event_sender.send(ChatEvent::MessageAdded {
                    chat_id,
                    message: assistant_message,
                });
                let finish_reason = stream_result.finish_reason.unwrap_or_else(|| "stop".to_string());
                let _ = event_sender.send(ChatEvent::StreamFinished {
                    chat_id,
                    message_id: assistant_message_id,
                    finish_reason,
                });
                Ok(assistant_message_id)
            }
            Err(rhd_ai::client::AiError::Aborted { .. }) => {
                let _ = event_sender.send(ChatEvent::StreamError {
                    chat_id,
                    error: "aborted".to_string(),
                });
                Err(ChatError::Ai(rhd_ai::client::AiError::Aborted {
                    model: model.to_string(),
                }))
            }
            Err(err) => {
                let _ = event_sender.send(ChatEvent::StreamError {
                    chat_id,
                    error: err.to_string(),
                });
                Err(ChatError::Ai(err))
            }
        }
    }

    pub async fn abort_chat(&self, chat_id: i64) -> bool {
        let active = self.active_streams.lock().await;
        if let Some(state) = active.get(&chat_id) {
            state.cancel_token().cancel();
            true
        } else {
            false
        }
    }

    pub async fn pause_chat(&self, chat_id: i64) -> bool {
        let mut active = self.active_streams.lock().await;
        if let Some(StreamState::Running { cancel_token, pause_notify }) = active.get(&chat_id) {
            let cancel_token = cancel_token.clone();
            let pause_notify = pause_notify.clone();
            active.insert(
                chat_id,
                StreamState::Paused {
                    cancel_token,
                    pause_notify,
                },
            );
            true
        } else {
            false
        }
    }

    pub async fn resume_chat(&self, chat_id: i64) -> bool {
        let mut active = self.active_streams.lock().await;
        if let Some(StreamState::Paused { cancel_token, pause_notify }) = active.get(&chat_id) {
            let cancel_token = cancel_token.clone();
            let pause_notify = pause_notify.clone();
            pause_notify.notify_one();
            active.insert(
                chat_id,
                StreamState::Running {
                    cancel_token,
                    pause_notify,
                },
            );
            true
        } else {
            false
        }
    }

    pub async fn attach_project(
        &self,
        chat_id: i64,
        project_name: &str,
        project_manager: &ProjectManager,
        mcp_cache: &McpServerCache,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<(), ChatError> {
        if self.db.get_chat(chat_id)?.is_none() {
            return Err(ChatError::ChatNotFound);
        }

        if project_manager.get_project(project_name).is_none() {
            return Err(ChatError::ProjectNotFound(project_name.to_string()));
        }

        project_manager
            .spawn_project_mcp(project_name, mcp_cache)
            .await
            .map_err(|e| ChatError::McpNotConnected(e))?;

        self.db.attach_project(chat_id, project_name)?;

        let _ = event_sender.send(ChatEvent::ProjectAttached {
            chat_id,
            project_name: project_name.to_string(),
        });

        Ok(())
    }

    pub async fn detach_project(
        &self,
        chat_id: i64,
        project_name: &str,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<(), ChatError> {
        self.db.detach_project(chat_id, project_name)?;

        let _ = event_sender.send(ChatEvent::ProjectDetached {
            chat_id,
            project_name: project_name.to_string(),
        });

        Ok(())
    }

    pub fn get_chat_projects(&self, chat_id: i64) -> Result<Vec<ProjectInfo>, ChatError> {
        let projects = self.db.get_chat_projects(chat_id)?;
        let mut infos = Vec::new();
        for (name, _system_prompt_added) in projects {
            infos.push(ProjectInfo {
                name,
                has_mcp: true,
                has_system_prompt: true,
            });
        }
        Ok(infos)
    }
}
