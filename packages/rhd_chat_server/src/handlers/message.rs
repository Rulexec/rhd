//! Message operation handlers.

use chrono::{DateTime, Utc};
use serde_json::Value;

use rhd_chat_api::methods::{
    AddMessageParams, AddMessageResult, DeleteMessageParams, DeleteMessageResult,
    UpdateMessageParams, UpdateMessageResult, UpdateToolCallTagsParams, UpdateToolCallTagsResult,
};
use rhd_chat_api::protocol::Response;
use rhd_chat_api::ErrorResponse;
use rhd_db::{ChatDb, DbError};

use crate::error::ServerError;
use crate::events::{message_added_event, message_deleted_event, message_updated_event};
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

/// Handle `addMessage` request.
pub async fn add_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: AddMessageParams = match serde_json::from_value(params) {
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

    // Add message
    let (message_id, chat_version) = db.add_message(
        params.chat_id,
        &params.role,
        &params.content,
        None, // model
        params.reasoning_content.as_deref(),
        params.is_finished,
        params.is_streaming,
    )?;

    // Set tags if provided
    let mut current_version = chat_version;
    if !params.tags.is_empty() {
        current_version = db.set_message_tags(message_id, &params.tags)?;
    }

    // Broadcast messageAdded event
    let msg_tags = db.get_message_tags(message_id)?;
    let db_message = db.get_message(message_id)?.unwrap();
    let message = convert_message_to_api(db_message, msg_tags)?;
    let event = message_added_event(params.chat_id, message, current_version);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat(params.chat_id, event);

    let result = AddMessageResult { message_id };
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}

/// Handle `updateMessage` request.
pub async fn update_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: UpdateMessageParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Check if message exists
    let message = match db.get_message(params.message_id)? {
        Some(m) => m,
        None => {
            return Ok(serde_json::to_value(ErrorResponse::message_not_found(request_id, params.message_id))?);
        }
    };

    // Update message fields if provided
    let mut current_version = db.get_chat(message.chat_id)?.map(|c| c.version).unwrap_or(1);
    if params.content.is_some()
        || params.reasoning_content.is_some()
        || params.tool_calls.is_some()
        || params.is_finished.is_some()
        || params.is_streaming.is_some()
    {
        current_version = db.update_message(
            params.message_id,
            params.content.as_deref(),
            params.reasoning_content.as_deref(),
            params.tool_calls.as_deref(),
            params.is_finished,
            params.is_streaming,
        )?;
    }

    // Add tags if provided
    if !params.add_tags.is_empty() {
        current_version = db.add_message_tags(params.message_id, &params.add_tags)?;
    }

    // Remove tags if provided
    if !params.remove_tags.is_empty() {
        current_version = db.remove_message_tags(params.message_id, &params.remove_tags)?;
    }

    // Broadcast messageUpdated event
    let msg_tags = db.get_message_tags(params.message_id)?;
    let db_message = db.get_message(params.message_id)?.unwrap();
    let api_message = convert_message_to_api(db_message, msg_tags)?;
    let chat_id = api_message.chat_id;
    let event = message_updated_event(chat_id, api_message, current_version);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat(chat_id, event);

    let result = UpdateMessageResult {};
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}

/// Handle `deleteMessage` request.
pub async fn delete_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: DeleteMessageParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Check if message exists
    let message = match db.get_message(params.message_id)? {
        Some(m) => m,
        None => {
            return Ok(serde_json::to_value(ErrorResponse::message_not_found(request_id, params.message_id))?);
        }
    };

    // Delete message (tags will be deleted by CASCADE)
    let chat_version = db.delete_message(params.message_id)?;

    // Broadcast messageDeleted event after deletion
    let event = message_deleted_event(message.chat_id, params.message_id, chat_version);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat(message.chat_id, event);

    let result = DeleteMessageResult {};
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}

/// Handle `updateToolCallTags` request.
///
/// Reads the message's stored tool calls, parses them, adds/removes tags on
/// the tool call with the given id, saves the result, and broadcasts a
/// `messageUpdated` event so subscribers can sync their state.
pub async fn update_tool_call_tags(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: UpdateToolCallTagsParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Check if message exists
    if db.get_message(params.message_id)?.is_none() {
        return Ok(serde_json::to_value(ErrorResponse::message_not_found(
            request_id,
            params.message_id,
        ))?);
    }

    // Apply tag changes; an unknown tool call id is a client error
    let chat_version = match db.update_message_tool_call_tags(
        params.message_id,
        &params.tool_call_id,
        &params.add_tags,
        &params.remove_tags,
    ) {
        Ok(version) => version,
        Err(DbError::NotFound(_)) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!(
                    "Tool call {} not found in message {}",
                    params.tool_call_id, params.message_id
                ),
            ))?);
        }
        Err(e) => return Err(e.into()),
    };

    // Broadcast messageUpdated event with the updated tool call tags
    let msg_tags = db.get_message_tags(params.message_id)?;
    let db_message = db.get_message(params.message_id)?.unwrap();
    let api_message = convert_message_to_api(db_message, msg_tags)?;
    let chat_id = api_message.chat_id;
    let event = message_updated_event(chat_id, api_message, chat_version);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat(chat_id, event);

    let result = UpdateToolCallTagsResult {};
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}
