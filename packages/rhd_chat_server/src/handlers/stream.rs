//! Stream operation handlers.

use serde_json::Value;

use rhd_chat_api::methods::{
    StreamFinishParams, StreamFinishResult, StreamPushParams, StreamPushResult,
    StreamSubscribeParams, StreamSubscribeResult,
};
use rhd_chat_api::protocol::{Event, Response};
use rhd_chat_api::ErrorResponse;

use crate::error::ServerError;
use crate::streams::{SharedStreamManager, StreamChunk};
use crate::subscriptions::SharedSubscriptionManager;

/// Handle `streamPush` request.
pub async fn stream_push(
    params: Value,
    request_id: &str,
    stream_manager: &SharedStreamManager,
    subscription_manager: &SharedSubscriptionManager,
    chat_id: i64,
) -> Result<Value, ServerError> {
    let params: StreamPushParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Extract fields before moving into push
    let reasoning_content = params.reasoning_content.clone();
    let content = params.content.clone();
    let tool_calls = params.tool_calls.clone();

    // Convert tool call deltas (they're already the same type now)
    let tool_calls_delta = params.tool_calls;

    // Push to stream
    stream_manager
        .push(chat_id, reasoning_content.clone(), content.clone(), tool_calls_delta)
        .await;

    // Broadcast streamChunk event to chat subscribers
    let manager = subscription_manager.read().await;
    if let Some(reasoning) = &reasoning_content {
        let event = Event::new(
            "streamChunk",
            serde_json::json!({
                "chatId": chat_id,
                "type": "reasoningDelta",
                "content": reasoning,
            }),
        );
        manager.broadcast_to_chat(chat_id, event);
    }
    if let Some(content) = &content {
        let event = Event::new(
            "streamChunk",
            serde_json::json!({
                "chatId": chat_id,
                "type": "contentDelta",
                "content": content,
            }),
        );
        manager.broadcast_to_chat(chat_id, event);
    }
    if let Some(tool_calls) = &tool_calls {
        let event = Event::new(
            "streamChunk",
            serde_json::json!({
                "chatId": chat_id,
                "type": "toolCallDelta",
                "toolCalls": tool_calls,
            }),
        );
        manager.broadcast_to_chat(chat_id, event);
    }

    let result = StreamPushResult { success: true };
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}

/// Handle `streamSubscribe` request.
pub async fn stream_subscribe(
    params: Value,
    request_id: &str,
    connection_id: &str,
    stream_manager: &SharedStreamManager,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: StreamSubscribeParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    let (snapshot, mut receiver) = stream_manager.subscribe_and_get(params.chat_id).await;

    // Spawn task to forward stream chunks to the connection
    let sub_manager = subscription_manager.clone();
    let conn_id = connection_id.to_string();
    let chat_id = params.chat_id;
    tokio::spawn(async move {
        while let Some(chunk) = receiver.recv().await {
            let event = match chunk {
                StreamChunk::ReasoningDelta { content } => Event::new(
                    "streamChunk",
                    serde_json::json!({
                        "chatId": chat_id,
                        "type": "reasoningDelta",
                        "content": content,
                    }),
                ),
                StreamChunk::ContentDelta { content } => Event::new(
                    "streamChunk",
                    serde_json::json!({
                        "chatId": chat_id,
                        "type": "contentDelta",
                        "content": content,
                    }),
                ),
                StreamChunk::ToolCallDelta { tool_calls } => Event::new(
                    "streamChunk",
                    serde_json::json!({
                        "chatId": chat_id,
                        "type": "toolCallDelta",
                        "toolCalls": tool_calls,
                    }),
                ),
                StreamChunk::Finished => {
                    let manager = sub_manager.read().await;
                    let event = Event::new(
                        "streamFinished",
                        serde_json::json!({ "chatId": chat_id }),
                    );
                    manager.send_to_connection(&conn_id, event);
                    break;
                }
            };
            let manager = sub_manager.read().await;
            manager.send_to_connection(&conn_id, event);
        }
    });

    let result = StreamSubscribeResult {
        reasoning_content: snapshot.reasoning_content,
        content: snapshot.content,
        tool_calls: snapshot.tool_calls,
        is_finished: snapshot.is_finished,
    };
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}

/// Handle `streamFinish` request.
pub async fn stream_finish(
    params: Value,
    request_id: &str,
    stream_manager: &SharedStreamManager,
    subscription_manager: &SharedSubscriptionManager,
    chat_id: i64,
) -> Result<Value, ServerError> {
    let _params: StreamFinishParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    let _snapshot = stream_manager.finish(chat_id).await;

    // Broadcast streamFinished event to chat subscribers
    let manager = subscription_manager.read().await;
    let event = Event::new(
        "streamFinished",
        serde_json::json!({ "chatId": chat_id }),
    );
    manager.broadcast_to_chat(chat_id, event);

    let result = StreamFinishResult { success: true };
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}
