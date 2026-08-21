//! Subscription operation handlers.

use serde_json::Value;

use rhd_chat_api::methods::{
    SubscribeChatParams, SubscribeChatResult, SubscribeChatsListResult, UnsubscribeChatParams,
    UnsubscribeChatResult, UnsubscribeChatsListResult,
};
use rhd_chat_api::protocol::Response;
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;
use crate::subscriptions::SharedSubscriptionManager;

/// Handle `subscribeChat` request.
pub async fn subscribe_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: SubscribeChatParams = match serde_json::from_value(params) {
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

    // Subscribe
    let mut manager = subscription_manager.write().await;
    manager.subscribe_chat(connection_id, params.chat_id);

    let result = SubscribeChatResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `unsubscribeChat` request.
pub async fn unsubscribe_chat(
    params: Value,
    _db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: UnsubscribeChatParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Unsubscribe
    let mut manager = subscription_manager.write().await;
    manager.unsubscribe_chat(connection_id, params.chat_id);

    let result = UnsubscribeChatResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `subscribeChatsList` request.
pub async fn subscribe_chats_list(
    _params: Value,
    _db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    // Subscribe
    let mut manager = subscription_manager.write().await;
    manager.subscribe_chats_list(connection_id);

    let result = SubscribeChatsListResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `unsubscribeChatsList` request.
pub async fn unsubscribe_chats_list(
    _params: Value,
    _db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    // Unsubscribe
    let mut manager = subscription_manager.write().await;
    manager.unsubscribe_chats_list(connection_id);

    let result = UnsubscribeChatsListResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}
