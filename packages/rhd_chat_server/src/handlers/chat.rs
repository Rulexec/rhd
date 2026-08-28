//! Chat operation handlers.

use chrono::{DateTime, Utc};
use serde_json::Value;

use rhd_chat_api::common::{Chat, ChatSummary};
use rhd_chat_api::methods::{
    CreateChatParams, CreateChatResult, DeleteChatParams, DeleteChatResult, GetChatParams,
    GetChatResult, ListChatsParams, ListChatsResult, UpdateChatParams, UpdateChatResult,
};
use rhd_chat_api::protocol::Response;
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;
use crate::events::{chat_created_event, chat_deleted_event, chat_updated_event};
use crate::subscriptions::SharedSubscriptionManager;

/// Convert rhd_db::ChatInfo to rhd_chat_api::Chat.
fn convert_chat_info_to_api(
    chat_info: rhd_db::ChatInfo,
    tags: Vec<String>,
) -> Result<Chat, ServerError> {
    let created_at: DateTime<Utc> = chat_info
        .created_at
        .parse()
        .map_err(|e| ServerError::Internal(format!("Failed to parse created_at: {}", e)))?;
    let updated_at: DateTime<Utc> = chat_info
        .updated_at
        .parse()
        .map_err(|e| ServerError::Internal(format!("Failed to parse updated_at: {}", e)))?;

    Ok(Chat {
        id: chat_info.id,
        title: chat_info.title,
        created_at,
        updated_at,
        tags,
        version: chat_info.version,
    })
}

/// Convert rhd_db::ChatInfo to rhd_chat_api::ChatSummary.
fn convert_chat_info_to_summary(
    chat_info: rhd_db::ChatInfo,
    tags: Vec<String>,
) -> Result<ChatSummary, ServerError> {
    let created_at: DateTime<Utc> = chat_info
        .created_at
        .parse()
        .map_err(|e| ServerError::Internal(format!("Failed to parse created_at: {}", e)))?;
    let updated_at: DateTime<Utc> = chat_info
        .updated_at
        .parse()
        .map_err(|e| ServerError::Internal(format!("Failed to parse updated_at: {}", e)))?;

    Ok(ChatSummary {
        id: chat_info.id,
        title: chat_info.title,
        created_at,
        updated_at,
        tags,
        version: chat_info.version,
    })
}

/// Convert rhd_db::Message to rhd_chat_api::Message.
fn convert_message_to_api(
    msg: rhd_db::Message,
    tags: Vec<String>,
) -> Result<rhd_chat_api::Message, ServerError> {
    let created_at: DateTime<Utc> = msg
        .created_at
        .parse()
        .map_err(|e| ServerError::Internal(format!("Failed to parse created_at: {}", e)))?;

    Ok(rhd_chat_api::Message {
        id: msg.id,
        chat_id: msg.chat_id,
        role: msg.role,
        content: msg.content,
        created_at,
        reasoning_content: msg.thinking_content,
        tags,
        is_finished: msg.is_finished,
        is_streaming: msg.is_streaming,
        tool_calls: msg
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(super::convert_tool_call_to_api)
            .collect(),
    })
}

/// Handle `createChat` request.
pub async fn create_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: CreateChatParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Create chat
    let chat_id = db.create_chat(&params.title)?;

    // Set tags if provided
    if !params.tags.is_empty() {
        db.set_chat_tags(chat_id, &params.tags)?;
    }

    // Broadcast chatCreated event
    let chat_tags = db.get_chat_tags(chat_id)?;
    let chat_info = db.get_chat(chat_id)?.unwrap();
    let chat_version = chat_info.version;
    let chat_summary = convert_chat_info_to_summary(chat_info, chat_tags)?;
    let event = chat_created_event(chat_summary, chat_version);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chats_list(event);

    // Return result
    let result = CreateChatResult { chat_id };
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}

/// Handle `listChats` request.
pub async fn list_chats(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Value, ServerError> {
    let params: ListChatsParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Get all chats
    let chat_infos = db.list_chats()?;

    // Convert to API types with tags
    let mut chats = Vec::new();
    for chat_info in chat_infos {
        // If tags filter is provided, check if chat has any of the tags
        if !params.tags.is_empty() {
            let chat_tags = db.get_chat_tags(chat_info.id)?;
            let has_matching_tag = params.tags.iter().any(|t| chat_tags.contains(t));
            if !has_matching_tag {
                continue;
            }
            let summary = convert_chat_info_to_summary(chat_info, chat_tags)?;
            chats.push(summary);
        } else {
            let chat_tags = db.get_chat_tags(chat_info.id)?;
            let summary = convert_chat_info_to_summary(chat_info, chat_tags)?;
            chats.push(summary);
        }
    }

    let result = ListChatsResult { chats };
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}

/// Handle `getChat` request.
pub async fn get_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Value, ServerError> {
    let params: GetChatParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Get chat info
    let chat_info = match db.get_chat(params.chat_id)? {
        Some(c) => c,
        None => {
            return Ok(serde_json::to_value(ErrorResponse::chat_not_found(request_id, params.chat_id))?);
        }
    };

    // Get chat tags
    let chat_tags = db.get_chat_tags(params.chat_id)?;
    let chat = convert_chat_info_to_api(chat_info, chat_tags)?;

    // Get messages
    let db_messages = db.get_messages(params.chat_id)?;
    let mut messages = Vec::new();
    for msg in db_messages {
        let msg_tags = db.get_message_tags(msg.id)?;
        let api_msg = convert_message_to_api(msg, msg_tags)?;
        messages.push(api_msg);
    }

    // Get queued messages count
    let queued_messages_count = db.count_queue_messages(params.chat_id)?;

    let result = GetChatResult {
        chat,
        messages,
        queued_messages_count,
        status: None,
    };
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}

/// Handle `deleteChat` request.
pub async fn delete_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: DeleteChatParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Check if chat exists
    if db.get_chat(params.chat_id)?.is_none() {
        return Ok(serde_json::to_value(ErrorResponse::chat_not_found(request_id, params.chat_id))?);
    }

    // Get chat version before deletion
    let chat_version = db.get_chat(params.chat_id)?.map(|c| c.version).unwrap_or(0);

    // Broadcast chatDeleted event before deletion
    let event = chat_deleted_event(params.chat_id, chat_version);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat_and_list(params.chat_id, event);

    // Delete chat (tags will be deleted by CASCADE)
    db.delete_chat(params.chat_id)?;

    let result = DeleteChatResult {};
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}

/// Handle `updateChat` request.
pub async fn update_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: UpdateChatParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Check if chat exists
    if db.get_chat(params.chat_id)?.is_none() {
        return Ok(serde_json::to_value(ErrorResponse::chat_not_found(request_id, params.chat_id))?);
    }

    // Update title if provided
    if let Some(title) = params.title {
        db.update_chat_title(params.chat_id, &title)?;
    }

    // Add tags if provided
    if !params.add_tags.is_empty() {
        db.add_chat_tags(params.chat_id, &params.add_tags)?;
    }

    // Remove tags if provided
    if !params.remove_tags.is_empty() {
        db.remove_chat_tags(params.chat_id, &params.remove_tags)?;
    }

    // Touch chat to update updated_at and get new version
    let chat_version = db.touch_chat(params.chat_id)?;

    // Broadcast chatUpdated event
    let chat_tags = db.get_chat_tags(params.chat_id)?;
    let chat_info = db.get_chat(params.chat_id)?.unwrap();
    let chat_summary = convert_chat_info_to_summary(chat_info, chat_tags)?;
    let event = chat_updated_event(chat_summary, chat_version);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chats_list(event);

    let result = UpdateChatResult {};
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}
