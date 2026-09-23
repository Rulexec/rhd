//! Idempotent tool-call answering helpers.
//!
//! Every phase of this plugin answers (and guards answers to) tool calls
//! through this module, so the duplicate-answer discipline lives in exactly
//! one place. Phase 4 extends the module additively (in-flight `AnswerGuards`);
//! keep these free functions self-contained so that can be layered on top.

use rhd_chat_api::{AddMessageParams, GetChatParams};
use rhd_chat_client::{ChatClient, ClientError};

/// True if a `tool`-role message answering `tool_call_id` already exists in
/// `chat_id` (todo_list's has_tool_result pattern; guard against duplicate answers).
pub async fn has_tool_result(
    client: &ChatClient,
    chat_id: i64,
    tool_call_id: &str,
) -> Result<bool, ClientError> {
    let chat = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await?;
    Ok(chat
        .messages
        .iter()
        .any(|m| m.tool_call_id.as_deref() == Some(tool_call_id)))
}

/// Post a `tool`-role answer for `tool_call_id` in `chat_id`.
pub async fn answer_tool_call(
    client: &ChatClient,
    chat_id: i64,
    tool_call_id: &str,
    content: String,
) -> Result<(), ClientError> {
    // The client returns the new message id; callers only care about success.
    client
        .add_message(AddMessageParams {
            chat_id,
            role: "tool".to_string(),
            content,
            tool_call_id: Some(tool_call_id.to_string()),
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .map(|_result| ())
}
