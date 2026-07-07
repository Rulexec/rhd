use std::collections::HashMap;
use std::sync::Arc;

use rhd_ai::client::OpenAiClient;
use rhd_ai::config::ModelConfig;
use rhd_db::Message;
use tokio::sync::{broadcast, Mutex};

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
    if manager.db().get_chat(chat_id)?.is_none() {
        return Err(ChatError::ChatNotFound);
    }

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

    let (tool_defs, mcp_clients) =
        tools::collect_tools_from_projects(manager.db(), manager.project_provider(), chat_id).await;

    if !tool_defs.is_empty() {
        return send_message_with_tools(
            manager,
            chat_id,
            content,
            model,
            model_config,
            tool_defs,
            mcp_clients,
            event_sender,
        )
        .await;
    }

    manager.db().update_chat_active_model(chat_id, model)?;

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

    let messages = manager.db().get_messages(chat_id)?;
    let chat_messages = tools::build_chat_messages(&messages);

    let (cancel_token, _pause_notify) = manager.register_stream(chat_id).await;

    let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);
    let accumulated_content = Arc::new(Mutex::new(String::new()));
    let accumulated_thinking = Arc::new(Mutex::new(String::new()));
    let accumulated_clone = accumulated_content.clone();
    let thinking_clone = accumulated_thinking.clone();
    let sender_for_closure = event_sender.clone();

    let api_model = &model_config.model;
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
) -> Result<i64, ChatError> {
    const MAX_ITERATIONS: u32 = 20;

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

    manager.db().update_message(message_id, &new_content)?;
    manager.db().truncate_messages(chat_id, message_id)?;

    projects::inject_system_prompts(
        manager.db(),
        manager.project_provider(),
        chat_id,
        &event_sender,
    )
    .await?;

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

    let (cancel_token, _pause_notify) = manager.register_stream(chat_id).await;

    let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);
    let accumulated_content = Arc::new(Mutex::new(String::new()));
    let accumulated_thinking = Arc::new(Mutex::new(String::new()));
    let accumulated_clone = accumulated_content.clone();
    let thinking_clone = accumulated_thinking.clone();
    let sender_for_closure = event_sender.clone();

    let api_model = &model_config.model;
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
    )
    .await
}

async fn handle_stream_result<P: ProjectProvider>(
    manager: &ChatManager<P>,
    result: Result<rhd_ai::client::StreamResult, rhd_ai::client::AiError>,
    chat_id: i64,
    model: &str,
    accumulated_content: Arc<Mutex<String>>,
    accumulated_thinking: Arc<Mutex<String>>,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<i64, ChatError> {
    match result {
        Ok(stream_result) => {
            let full_content = accumulated_content.lock().await.clone();
            let full_thinking = accumulated_thinking.lock().await.clone();
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
            let finish_reason = stream_result
                .finish_reason
                .unwrap_or_else(|| "stop".to_string());
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
