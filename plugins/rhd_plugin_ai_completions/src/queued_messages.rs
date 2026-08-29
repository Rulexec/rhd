//! Promotion of queued messages into the conversation before a request is built.

use rhd_chat_api::{AddMessageParams, DeleteQueueMessageParams, GetQueueMessagesParams};
use rhd_chat_client::ChatClient;

use crate::ai_request::AiRequestError;

/// Process queued messages: remove from queue and add as regular messages.
///
/// `tool_call_id` is carried through (P8) so a queued `tool` message keeps the id of
/// the call it answers and passes the D4 integrity check after promotion.
pub async fn process_queued_messages(
    client: &ChatClient,
    chat_id: i64,
) -> Result<(), AiRequestError> {
    tracing::debug!(chat_id = chat_id, "fetching queued messages");
    // Get queued messages
    let queue_result = client
        .get_queue_messages(GetQueueMessagesParams { chat_id })
        .await
        .map_err(|e| AiRequestError::QueueGet(e.to_string()))?;

    tracing::debug!(
        chat_id = chat_id,
        message_count = queue_result.messages.len(),
        "found queued messages"
    );

    // Delete each queued message and add as regular message
    for queue_msg in queue_result.messages {
        tracing::debug!(
            chat_id = chat_id,
            message_id = queue_msg.id,
            "deleting message from queue"
        );
        // Delete from queue
        client
            .delete_queue_message(DeleteQueueMessageParams {
                message_id: queue_msg.id,
            })
            .await
            .map_err(|e| AiRequestError::QueueDelete(e.to_string()))?;

        tracing::debug!(
            chat_id = chat_id,
            message_id = queue_msg.id,
            role = %queue_msg.role,
            "adding message as regular message"
        );
        // Add as regular message
        client
            .add_message(AddMessageParams {
                chat_id,
                role: queue_msg.role,
                content: queue_msg.content,
                tool_call_id: queue_msg.tool_call_id,
                reasoning_content: queue_msg.reasoning_content,
                tags: queue_msg.tags,
                is_finished: true,
                is_streaming: false,
            })
            .await
            .map_err(|e| AiRequestError::MessageAdd(e.to_string()))?;
        tracing::debug!(
            chat_id = chat_id,
            message_id = queue_msg.id,
            "successfully added message"
        );
    }

    Ok(())
}
