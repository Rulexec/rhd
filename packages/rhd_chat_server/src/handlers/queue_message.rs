//! Queue message operation handlers.

use chrono::{DateTime, Utc};
use serde_json::Value;

use rhd_chat_api::methods::{
    AddQueueMessageParams, AddQueueMessageResult, DeleteQueueMessageParams, DeleteQueueMessageResult,
    GetQueueMessagesParams, GetQueueMessagesResult, UpdateQueueMessageParams, UpdateQueueMessageResult,
};
use rhd_chat_api::protocol::Response;
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;
use crate::events::{
    queue_message_added_event, queue_message_deleted_event, queue_message_updated_event,
};
use crate::subscriptions::SharedSubscriptionManager;

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
    })
}

/// Handle `addQueueMessage` request.
pub async fn add_queue_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: AddQueueMessageParams = match serde_json::from_value(params) {
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
        return Ok(serde_json::to_value(ErrorResponse::chat_not_found(
            request_id,
            params.chat_id,
        ))?);
    }

    // Add queue message
    let message_id = db.add_queue_message(
        params.chat_id,
        &params.role,
        &params.content,
        None, // model
        params.reasoning_content.as_deref(),
    )?;

    // Set tags if provided
    if !params.tags.is_empty() {
        db.set_queue_message_tags(message_id, &params.tags)?;
    }

    // Touch chat to update updated_at
    db.touch_chat(params.chat_id)?;

    // Broadcast queueMessageAdded event
    let msg_tags = db.get_queue_message_tags(message_id)?;
    let db_message = db.get_queue_message(message_id)?.unwrap();
    let message = convert_message_to_api(db_message, msg_tags)?;
    let event = queue_message_added_event(params.chat_id, message);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat(params.chat_id, event);

    let result = AddQueueMessageResult { message_id };
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `updateQueueMessage` request.
pub async fn update_queue_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: UpdateQueueMessageParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Check if queue message exists
    let message = match db.get_queue_message(params.message_id)? {
        Some(m) => m,
        None => {
            return Ok(serde_json::to_value(ErrorResponse::message_not_found(
                request_id,
                params.message_id,
            ))?);
        }
    };

    // Update content if provided
    if let Some(content) = params.content {
        db.update_queue_message(params.message_id, &content)?;
    }

    // Add tags if provided
    if !params.add_tags.is_empty() {
        db.add_queue_message_tags(params.message_id, &params.add_tags)?;
    }

    // Remove tags if provided
    if !params.remove_tags.is_empty() {
        db.remove_queue_message_tags(params.message_id, &params.remove_tags)?;
    }

    // Touch chat to update updated_at
    db.touch_chat(message.chat_id)?;

    // Broadcast queueMessageUpdated event
    let msg_tags = db.get_queue_message_tags(params.message_id)?;
    let db_message = db.get_queue_message(params.message_id)?.unwrap();
    let api_message = convert_message_to_api(db_message, msg_tags)?;
    let chat_id = api_message.chat_id;
    let event = queue_message_updated_event(chat_id, api_message);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat(chat_id, event);

    let result = UpdateQueueMessageResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `deleteQueueMessage` request.
pub async fn delete_queue_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: DeleteQueueMessageParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Check if queue message exists
    let message = match db.get_queue_message(params.message_id)? {
        Some(m) => m,
        None => {
            return Ok(serde_json::to_value(ErrorResponse::message_not_found(
                request_id,
                params.message_id,
            ))?);
        }
    };

    // Broadcast queueMessageDeleted event before deletion
    let event = queue_message_deleted_event(message.chat_id, params.message_id);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat(message.chat_id, event);

    // Delete queue message (tags will be deleted by CASCADE)
    db.delete_queue_message(params.message_id)?;

    // Touch chat to update updated_at
    db.touch_chat(message.chat_id)?;

    let result = DeleteQueueMessageResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `getQueueMessages` request.
pub async fn get_queue_messages(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Value, ServerError> {
    let params: GetQueueMessagesParams = match serde_json::from_value(params) {
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
        return Ok(serde_json::to_value(ErrorResponse::chat_not_found(
            request_id,
            params.chat_id,
        ))?);
    }

    // Get all queue messages
    let db_messages = db.get_queue_messages(params.chat_id)?;
    let mut messages = Vec::new();
    for msg in db_messages {
        let tags = db.get_queue_message_tags(msg.id)?;
        messages.push(convert_message_to_api(msg, tags)?);
    }

    let result = GetQueueMessagesResult { messages };
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}
