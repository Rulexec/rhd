use std::collections::HashMap;
use std::sync::Arc;

use rhd_ai::client::{OpenAiClient, RawLogger};
use rhd_ai::config::ModelConfig;
use rhd_db::Message;
use tokio::sync::{broadcast, Mutex};

use crate::chat_log;
use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::projects;
use crate::tools;
use crate::ProjectProvider;

use crate::stream::TemplateLoaderRef;
use crate::stream::contract::inject_todo_tool_contract;
use super::tools::send_message_with_tools;
use super::utils::{format_messages_for_log, handle_stream_result};

pub async fn send_message<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    content: String,
    model: &str,
    models: &HashMap<String, ModelConfig>,
    event_sender: broadcast::Sender<ChatEvent>,
    reload_lock: &tokio::sync::RwLock<()>,
    template_loader: &TemplateLoaderRef,
) -> Result<i64, ChatError> {
    let _reload_guard = reload_lock.read().await;
    let chat_info = manager.db().get_chat(chat_id)?.ok_or(ChatError::ChatNotFound)?;
    let chat_title = chat_info.title.clone();

    inject_todo_tool_contract(manager, chat_id, template_loader, &event_sender)?;

    let pause_notify = manager.get_paused_notify(chat_id).await;

    if let Some(notify) = pause_notify {
        let user_message = manager.add_message_and_notify(chat_id, "user", &content, Some(model), None, &event_sender)?;
        let user_message_id = user_message.id;

        notify.notify_one();

        return Ok(user_message_id);
    }

    let model_config = models
        .get(model)
        .ok_or_else(|| ChatError::ModelNotFound(model.to_string()))?;

    projects::inject_system_prompts(manager, chat_id, &event_sender).await?;
    projects::inject_roles_prompt(manager, chat_id, &event_sender, template_loader).await?;
    projects::inject_pending_role_prompt(manager, chat_id, &event_sender, template_loader)?;

    manager.db().update_chat_active_model(chat_id, model)?;

    let user_message = manager.add_message_and_notify(chat_id, "user", &content, Some(model), None, &event_sender)?;
    let _user_message_id = user_message.id;

    let messages = manager.db().get_messages(chat_id)?;
    
    let (tool_defs, mcp_clients) =
        tools::collect_tools_from_projects(manager.db(), manager.project_provider(), chat_id, template_loader).await;

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
            template_loader,
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

pub async fn edit_and_resend<P: ProjectProvider>(
    manager: &ChatManager<P>,
    message_id: i64,
    new_content: String,
    model: &str,
    models: &HashMap<String, ModelConfig>,
    event_sender: broadcast::Sender<ChatEvent>,
    reload_lock: &tokio::sync::RwLock<()>,
    template_loader: &TemplateLoaderRef,
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

    inject_todo_tool_contract(manager, chat_id, template_loader, &event_sender)?;

    projects::inject_system_prompts(manager, chat_id, &event_sender).await?;
    projects::inject_roles_prompt(manager, chat_id, &event_sender, template_loader).await?;
    projects::inject_pending_role_prompt(manager, chat_id, &event_sender, template_loader)?;

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
