use std::sync::Arc;

use rhd_api::{ErrorCode, WsResponse};

use crate::daemon::DaemonState;

pub async fn handle_get_available_models(id: String, state: &Arc<DaemonState>) -> WsResponse {
    let inner = state.inner.read().await;
    let mut model_names: Vec<String> = inner
        .models
        .iter()
        .filter(|(_, config)| !config.is_alias)
        .map(|(name, _)| name.clone())
        .collect();
    model_names.sort();
    WsResponse::success(id, serde_json::to_value(&model_names).unwrap())
}

pub fn handle_create_chat(id: String, title: String, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.create_chat(&title) {
        Ok(chat_id) => WsResponse::success(id, serde_json::json!({ "chatId": chat_id })),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to create chat: {}", err),
        ),
    }
}

pub fn handle_list_chats(id: String, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.list_chats() {
        Ok(chats) => {
            let data = serde_json::to_value(&chats).unwrap_or(serde_json::json!([]));
            WsResponse::success(id, data)
        }
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to list chats: {}", err),
        ),
    }
}

pub fn handle_get_chat(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.get_chat(chat_id) {
        Ok(Some((chat_info, messages))) => {
            let data = serde_json::json!({
                "chat": chat_info,
                "messages": messages,
            });
            WsResponse::success(id, data)
        }
        Ok(None) => WsResponse::error(
            id,
            ErrorCode::ChatNotFound,
            format!("chat not found: {}", chat_id),
        ),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to get chat: {}", err),
        ),
    }
}

pub fn handle_delete_chat(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.delete_chat(chat_id) {
        Ok(()) => WsResponse::success(id, serde_json::json!({ "deleted": true })),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to delete chat: {}", err),
        ),
    }
}

pub async fn handle_delete_all_chats(id: String, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.delete_all_chats().await {
        Ok(()) => WsResponse::success(id, serde_json::json!({ "deleted": true })),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to delete all chats: {}", err),
        ),
    }
}

pub async fn handle_send_message(
    id: String,
    chat_id: i64,
    content: String,
    model: String,
    state: &Arc<DaemonState>,
) -> WsResponse {
    let chat_manager = Arc::clone(&state.chat_manager);
    let inner = state.inner.read().await;
    let models = inner.models.clone();
    drop(inner);
    let event_sender = state.chat_event_sender.clone();
    let template_loader = Arc::clone(&state.template_loader);
    
    let state_clone = Arc::clone(state);
    tokio::spawn(async move {
        let template_loader_ref = rhd_chat::stream::TemplateLoaderRef::new(move |name| {
            template_loader.get_template(name).map(|s| s.to_string())
        });
        let _ = chat_manager
            .send_message(chat_id, content, &model, &models, event_sender, &state_clone.reload_lock, &template_loader_ref)
            .await;
    });
    
    WsResponse::success(id, serde_json::json!({ "status": "sending" }))
}

pub async fn handle_edit_message(
    id: String,
    message_id: i64,
    content: String,
    model: String,
    state: &Arc<DaemonState>,
) -> WsResponse {
    let inner = state.inner.read().await;
    let models = inner.models.clone();
    drop(inner);
    let template_loader = Arc::clone(&state.template_loader);
    let template_loader_ref = rhd_chat::stream::TemplateLoaderRef::new(move |name| {
        template_loader.get_template(name).map(|s| s.to_string())
    });
    match state
        .chat_manager
        .edit_and_resend(
            message_id,
            content,
            &model,
            &models,
            state.chat_event_sender.clone(),
            &state.reload_lock,
            &template_loader_ref,
        )
        .await
    {
        Ok(new_message_id) => {
            WsResponse::success(id, serde_json::json!({ "messageId": new_message_id }))
        }
        Err(rhd_chat::ChatError::MessageNotFound) => WsResponse::error(
            id,
            ErrorCode::MessageNotFound,
            format!("message not found: {}", message_id),
        ),
        Err(rhd_chat::ChatError::ModelNotFound(model_name)) => WsResponse::error(
            id,
            ErrorCode::InvalidRequest,
            format!("model not found: {}", model_name),
        ),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::ChatStreamFailed,
            format!("failed to edit message: {}", err),
        ),
    }
}

pub async fn handle_abort_chat(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    let aborted = state.chat_manager.abort_chat(chat_id).await;
    WsResponse::success(id, serde_json::json!({ "aborted": aborted }))
}

pub fn handle_dev_notification(id: String, state: &Arc<DaemonState>) -> WsResponse {
    let _ = state.chat_event_sender.send(rhd_chat::ChatEvent::DevNotification {
        title: "RHD Test".to_string(),
        message: "This is a test notification from rhd dev frontend-notification".to_string(),
    });
    WsResponse::success(id, serde_json::json!({ "sent": true }))
}

pub async fn handle_pause_chat(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    let paused = state.chat_manager.pause_chat(chat_id).await;
    WsResponse::success(id, serde_json::json!({ "paused": paused }))
}

pub async fn handle_resume_chat(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    let resumed = state.chat_manager.resume_chat(chat_id).await;
    WsResponse::success(id, serde_json::json!({ "resumed": resumed }))
}
