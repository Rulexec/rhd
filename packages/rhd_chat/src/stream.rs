use std::collections::HashMap;
use std::sync::Arc;

use rhd_ai::client::{OpenAiClient, RawLogger};
use rhd_ai::config::ModelConfig;
use rhd_db::Message;
use tokio::sync::{broadcast, Mutex};

use crate::chat_log::{self, ChatLogSink};
use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::projects;
use crate::tools;
use crate::ProjectProvider;

pub async fn send_message<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    content: String,
    model: &str,
    models: &HashMap<String, ModelConfig>,
    event_sender: broadcast::Sender<ChatEvent>,
    reload_lock: &tokio::sync::RwLock<()>,
) -> Result<i64, ChatError> {
    let _reload_guard = reload_lock.read().await;
    let chat_info = manager.db().get_chat(chat_id)?.ok_or(ChatError::ChatNotFound)?;
    let chat_title = chat_info.title.clone();

    let pause_notify = manager.get_paused_notify(chat_id).await;

    if let Some(notify) = pause_notify {
        let user_message_id =
            manager.db().add_message(chat_id, "user", &content, Some(model), None)?;
        let user_message = Message {
            id: user_message_id,
            chat_id,
            role: "user".to_string(),
            content,
            created_at: chrono::Utc::now().to_rfc3339(),
            model: Some(model.to_string()),
            thinking_content: None,
        };
        let _ = event_sender.send(ChatEvent::MessageAdded {
            chat_id,
            message: user_message,
        });

        notify.notify_one();

        return Ok(user_message_id);
    }

    let model_config = models
        .get(model)
        .ok_or_else(|| ChatError::ModelNotFound(model.to_string()))?;

    projects::inject_system_prompts(
        manager.db(),
        manager.project_provider(),
        chat_id,
        &event_sender,
    )
    .await?;

    projects::inject_roles_prompt(
        manager.db(),
        manager.project_provider(),
        chat_id,
        &event_sender,
    )
    .await?;

    projects::inject_pending_role_prompt(
        manager.db(),
        manager.project_provider(),
        chat_id,
        &event_sender,
    )?;

    manager.db().update_chat_active_model(chat_id, model)?;

    let user_message_id =
        manager.db().add_message(chat_id, "user", &content, Some(model), None)?;
    let user_message = Message {
        id: user_message_id,
        chat_id,
        role: "user".to_string(),
        content: content.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        model: Some(model.to_string()),
        thinking_content: None,
    };
    let _ = event_sender.send(ChatEvent::MessageAdded {
        chat_id,
        message: user_message,
    });

    let messages = manager.db().get_messages(chat_id)?;
    
    let (tool_defs, mcp_clients) =
        tools::collect_tools_from_projects(manager.db(), manager.project_provider(), chat_id).await;

    if !tool_defs.is_empty() {
        let tool_names: Vec<String> = tool_defs.iter().map(|t| t.function.name.clone()).collect();
        
        let mut loggers = if let Some(log_chats_dir) = manager.log_chats() {
            match chat_log::create_chat_loggers(log_chats_dir, &chat_title, manager.log_chats_raw()) {
                Ok(l) => Some(l),
                Err(_) => None,
            }
        } else {
            None
        };

        if let Some(ref mut l) = loggers {
            let messages_str = format_messages_for_log(&messages);
            l.chat_log.log_stream_start(chat_id, &chat_title, model, &tool_names, &messages_str);
        }

        return send_message_with_tools(
            manager,
            chat_id,
            content,
            model,
            model_config,
            tool_defs,
            mcp_clients,
            event_sender,
            loggers,
        )
        .await;
    }

    let chat_messages = tools::build_chat_messages(&messages);

    let mut loggers = if let Some(log_chats_dir) = manager.log_chats() {
        match chat_log::create_chat_loggers(log_chats_dir, &chat_title, manager.log_chats_raw()) {
            Ok(l) => Some(l),
            Err(_) => None,
        }
    } else {
        None
    };

    if let Some(ref mut l) = loggers {
        let messages_str = format_messages_for_log(&messages);
        l.chat_log.log_stream_start(chat_id, &chat_title, model, &[], &messages_str);
    }

    let (cancel_token, _pause_notify) = manager.register_stream(chat_id).await;

    let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);
    let accumulated_content = Arc::new(Mutex::new(String::new()));
    let accumulated_thinking = Arc::new(Mutex::new(String::new()));
    let accumulated_clone = accumulated_content.clone();
    let thinking_clone = accumulated_thinking.clone();
    let sender_for_closure = event_sender.clone();

    let api_model = &model_config.model;
    let raw_log_ref = loggers.as_mut().and_then(|l| l.raw_log.as_mut().map(|r| r as &mut dyn RawLogger));
    let result = client
        .chat_stream_cancellable(
            api_model,
            &chat_messages,
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

    manager.unregister_stream(chat_id).await;

    handle_stream_result(
        manager,
        result,
        chat_id,
        model,
        accumulated_content,
        accumulated_thinking,
        &event_sender,
        loggers.as_mut().map(|l| &mut l.chat_log),
    )
    .await
}

async fn send_message_with_tools<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    content: String,
    model: &str,
    model_config: &ModelConfig,
    tool_defs: Vec<rhd_ai::ToolDefinition>,
    mcp_clients: Vec<(
        String,
        String,
        Arc<rhd_mcp_client::client::McpClient>,
    )>,
    event_sender: broadcast::Sender<ChatEvent>,
    loggers: Option<chat_log::ChatLoggers>,
) -> Result<i64, ChatError> {
    const MAX_ITERATIONS: u32 = 20;

    let (cancel_token, _pause_notify) = manager.register_stream(chat_id).await;

    let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);
    let mut iterations = 0u32;
    let mut current_content = content;

    let result = tools::tool_loop(
        manager,
        chat_id,
        model,
        &model_config.model,
        &client,
        &tool_defs,
        &mcp_clients,
        &cancel_token,
        &event_sender,
        &mut iterations,
        &mut current_content,
        MAX_ITERATIONS,
        loggers,
    )
    .await;

    manager.unregister_stream(chat_id).await;

    result
}

pub async fn edit_and_resend<P: ProjectProvider>(
    manager: &ChatManager<P>,
    message_id: i64,
    new_content: String,
    model: &str,
    models: &HashMap<String, ModelConfig>,
    event_sender: broadcast::Sender<ChatEvent>,
    reload_lock: &tokio::sync::RwLock<()>,
) -> Result<i64, ChatError> {
    let _reload_guard = reload_lock.read().await;
    let original_message = manager
        .db()
        .get_message(message_id)?
        .ok_or(ChatError::MessageNotFound)?;
    let chat_id = original_message.chat_id;
    
    let chat_info = manager.db().get_chat(chat_id)?.ok_or(ChatError::ChatNotFound)?;
    let chat_title = chat_info.title.clone();

    manager.db().update_message(message_id, &new_content)?;
    manager.db().truncate_messages(chat_id, message_id)?;

    projects::inject_system_prompts(
        manager.db(),
        manager.project_provider(),
        chat_id,
        &event_sender,
    )
    .await?;

    projects::inject_roles_prompt(
        manager.db(),
        manager.project_provider(),
        chat_id,
        &event_sender,
    )
    .await?;

    projects::inject_pending_role_prompt(
        manager.db(),
        manager.project_provider(),
        chat_id,
        &event_sender,
    )?;

    manager.db().update_chat_active_model(chat_id, model)?;

    let updated_message = Message {
        id: message_id,
        chat_id,
        role: original_message.role.clone(),
        content: new_content,
        created_at: chrono::Utc::now().to_rfc3339(),
        model: Some(model.to_string()),
        thinking_content: None,
    };
    let _ = event_sender.send(ChatEvent::MessageAdded {
        chat_id,
        message: updated_message,
    });

    let messages = manager.db().get_messages(chat_id)?;
    let chat_messages = tools::build_chat_messages(&messages);

    let model_config = models
        .get(model)
        .ok_or_else(|| ChatError::ModelNotFound(model.to_string()))?;

    let mut loggers = if let Some(log_chats_dir) = manager.log_chats() {
        match chat_log::create_chat_loggers(log_chats_dir, &chat_title, manager.log_chats_raw()) {
            Ok(l) => Some(l),
            Err(_) => None,
        }
    } else {
        None
    };

    if let Some(ref mut l) = loggers {
        let messages_str = format_messages_for_log(&messages);
        l.chat_log.log_stream_start(chat_id, &chat_title, model, &[], &messages_str);
    }

    let (cancel_token, _pause_notify) = manager.register_stream(chat_id).await;

    let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);
    let accumulated_content = Arc::new(Mutex::new(String::new()));
    let accumulated_thinking = Arc::new(Mutex::new(String::new()));
    let accumulated_clone = accumulated_content.clone();
    let thinking_clone = accumulated_thinking.clone();
    let sender_for_closure = event_sender.clone();

    let api_model = &model_config.model;
    let raw_log_ref = loggers.as_mut().and_then(|l| l.raw_log.as_mut().map(|r| r as &mut dyn RawLogger));
    let result = client
        .chat_stream_cancellable(
            api_model,
            &chat_messages,
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

    manager.unregister_stream(chat_id).await;

    handle_stream_result(
        manager,
        result,
        chat_id,
        model,
        accumulated_content,
        accumulated_thinking,
        &event_sender,
        loggers.as_mut().map(|l| &mut l.chat_log),
    )
    .await
}

fn format_messages_for_log(messages: &[Message]) -> String {
    let mut output = String::new();
    for msg in messages {
        match msg.role.as_str() {
            "system" => {
                output.push_str(&format!("[system] {}\n", msg.content));
            }
            "user" => {
                output.push_str(&format!("[user] {}\n", msg.content));
            }
            "assistant" => {
                output.push_str(&format!("[assistant] {}\n", msg.content));
            }
            "tool" => {
                output.push_str(&format!("[tool {}] {}\n", msg.id, msg.content));
            }
            _ => {
                output.push_str(&format!("[{}] {}\n", msg.role, msg.content));
            }
        }
    }
    output
}

async fn handle_stream_result<P: ProjectProvider>(
    manager: &ChatManager<P>,
    result: Result<rhd_ai::client::StreamResult, rhd_ai::client::AiError>,
    chat_id: i64,
    model: &str,
    accumulated_content: Arc<Mutex<String>>,
    accumulated_thinking: Arc<Mutex<String>>,
    event_sender: &broadcast::Sender<ChatEvent>,
    chat_log: Option<&mut ChatLogSink>,
) -> Result<i64, ChatError> {
    match result {
        Ok(stream_result) => {
            let full_content = accumulated_content.lock().await.clone();
            let full_thinking = accumulated_thinking.lock().await.clone();
            let finish_reason = stream_result
                .finish_reason
                .clone()
                .unwrap_or_else(|| "stop".to_string());
            
            if let Some(sink) = chat_log {
                let reasoning = if full_thinking.is_empty() {
                    None
                } else {
                    Some(full_thinking.as_str())
                };
                sink.log_assistant_response(
                    reasoning,
                    &full_content,
                    &finish_reason,
                    stream_result.usage.as_ref(),
                );
                sink.log_stream_finished(&finish_reason, 0);
            }
            
            let thinking_option = if full_thinking.is_empty() {
                None
            } else {
                Some(full_thinking.as_str())
            };
            let assistant_message_id = manager.db().add_message(
                chat_id,
                "assistant",
                &full_content,
                Some(model),
                thinking_option,
            )?;
            let assistant_message = Message {
                id: assistant_message_id,
                chat_id,
                role: "assistant".to_string(),
                content: full_content,
                created_at: chrono::Utc::now().to_rfc3339(),
                model: Some(model.to_string()),
                thinking_content: if full_thinking.is_empty() {
                    None
                } else {
                    Some(full_thinking)
                },
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
            Ok(assistant_message_id)
        }
        Err(rhd_ai::client::AiError::Aborted { .. }) => {
            if let Some(sink) = chat_log {
                sink.log_stream_error("aborted");
            }
            let _ = event_sender.send(ChatEvent::StreamError {
                chat_id,
                error: "aborted".to_string(),
            });
            Err(ChatError::Ai(rhd_ai::client::AiError::Aborted {
                model: model.to_string(),
            }))
        }
        Err(err) => {
            if let Some(sink) = chat_log {
                sink.log_stream_error(&err.to_string());
            }
            let _ = event_sender.send(ChatEvent::StreamError {
                chat_id,
                error: err.to_string(),
            });
            Err(ChatError::Ai(err))
        }
    }
}
