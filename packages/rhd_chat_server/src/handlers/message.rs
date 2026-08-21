//! Message operation handlers.

use serde_json::Value;

use rhd_chat_api::methods::{
    AddMessageParams, AddMessageResult, DeleteMessageParams, DeleteMessageResult,
    UpdateMessageParams, UpdateMessageResult,
};
use rhd_chat_api::protocol::Response;
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;

/// Handle `addMessage` request.
pub async fn add_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
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
    let message_id = db.add_message(
        params.chat_id,
        &params.role,
        &params.content,
        None, // model
        params.reasoning_content.as_deref(),
    )?;

    // Set tags if provided
    if !params.tags.is_empty() {
        db.set_message_tags(message_id, &params.tags)?;
    }

    // Touch chat to update updated_at
    db.touch_chat(params.chat_id)?;

    let result = AddMessageResult { message_id };
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}

/// Handle `updateMessage` request.
pub async fn update_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
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

    // Update content if provided
    if let Some(content) = params.content {
        db.update_message(params.message_id, &content)?;
    }

    // Note: rhd_db doesn't have methods to update reasoning_content or role separately
    // For now, we'll need to use update_message_full or add new methods to rhd_db
    // This is a limitation that should be addressed in a future phase

    // Add tags if provided
    if !params.add_tags.is_empty() {
        db.add_message_tags(params.message_id, &params.add_tags)?;
    }

    // Remove tags if provided
    if !params.remove_tags.is_empty() {
        db.remove_message_tags(params.message_id, &params.remove_tags)?;
    }

    // Touch chat to update updated_at
    db.touch_chat(message.chat_id)?;

    let result = UpdateMessageResult {};
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}

/// Handle `deleteMessage` request.
pub async fn delete_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
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
    db.delete_message(params.message_id)?;

    // Touch chat to update updated_at
    db.touch_chat(message.chat_id)?;

    let result = DeleteMessageResult {};
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}
