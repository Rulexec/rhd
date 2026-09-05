//! Executes MCP tool calls and pushes results back to the chat.

use std::sync::Arc;

use rhd_chat_api::{
    AddMessageParams, AssistantMessageWithToolCallsData, GetChatParams, ToolCall,
};
use rhd_chat_client::ChatClient;

use crate::mcp_pool::McpPool;
use crate::plugin::ChatRegistrations;

/// Handle an `assistantMessageWithToolCalls` event for all calls this plugin owns.
///
/// For each tool call in the message:
/// 1. Skip names this plugin cannot route (not our prefix).
/// 2. Skip servers not registered on this chat (AD-3; defensive).
/// 3. Skip calls already answered (duplicate guard).
/// 4. Route `{name}:{tool}` → owning server, execute via the pool.
/// 5. Add a `tool`-role message with `toolCallId` and the result content.
///    Errors become content too — the model decides how to react.
pub async fn handle_tool_calls(
    client: Arc<ChatClient>,
    pool: Arc<McpPool>,
    registrations: ChatRegistrations,
    event: AssistantMessageWithToolCallsData,
) {
    let chat_id = event.chat_id;

    for tool_call in &event.message.tool_calls {
        if let Err(e) =
            handle_single_call(&client, &pool, &registrations, chat_id, tool_call).await
        {
            // A failed call must still be answered so the AI tool loop can proceed.
            tracing::error!(
                chat_id = chat_id,
                tool = %tool_call.function.name,
                tool_call_id = %tool_call.id,
                error = %e,
                "failed to answer MCP tool call"
            );
        }
    }
}

async fn handle_single_call(
    client: &ChatClient,
    pool: &McpPool,
    registrations: &ChatRegistrations,
    chat_id: i64,
    tool_call: &ToolCall,
) -> Result<(), HandlerError> {
    let prefixed_name = &tool_call.function.name;

    // 1. Not routable by us (subscription filter should prevent this).
    let Some(route) = pool.route(prefixed_name) else {
        tracing::debug!(chat_id, tool = %prefixed_name, "tool call not owned by this plugin");
        return Ok(());
    };

    // 2. Server's tools must have been registered on this chat (AD-3).
    {
        let regs = registrations.read().await;
        let registered = regs
            .get(&chat_id)
            .map(|ids| ids.contains(&route.server_id))
            .unwrap_or(false);
        if !registered {
            tracing::warn!(
                chat_id,
                tool = %prefixed_name,
                server_id = %route.server_id,
                "tool call for server not registered on this chat, skipping"
            );
            return Ok(());
        }
    }

    // 3. Duplicate guard: a tool message with this tool_call_id may exist already.
    if has_tool_result(client, chat_id, &tool_call.id).await? {
        tracing::debug!(
            chat_id,
            tool_call_id = %tool_call.id,
            "tool result already exists, skipping"
        );
        return Ok(());
    }

    tracing::info!(
        chat_id,
        tool = %prefixed_name,
        tool_call_id = %tool_call.id,
        arguments = %tool_call.function.arguments,
        "executing MCP tool call"
    );

    // 4. Execute (serialized per server inside the pool, AD-5).
    let content = match pool
        .call_tool(
            &route.server_id,
            &route.tool_name,
            &tool_call.function.arguments,
        )
        .await
    {
        Ok(result) => {
            if result.is_error.unwrap_or(false) {
                tracing::warn!(
                    chat_id,
                    tool = %prefixed_name,
                    "MCP server reported tool error"
                );
            }
            result.content
        }
        Err(e) => {
            // 5a. Error path: surface as tool content, keep the loop alive.
            tracing::error!(chat_id, tool = %prefixed_name, error = %e, "MCP call failed");
            format!("MCP tool call failed: {}", e)
        }
    };

    // 5b. Push result.
    client
        .add_message(AddMessageParams {
            chat_id,
            role: "tool".to_string(),
            content,
            tool_call_id: Some(tool_call.id.clone()),
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .map_err(|e| HandlerError::AddMessage(e.to_string()))?;

    tracing::debug!(chat_id, tool_call_id = %tool_call.id, "tool result pushed");
    Ok(())
}

/// True if the chat already contains a `tool`-role message answering this call id.
async fn has_tool_result(
    client: &ChatClient,
    chat_id: i64,
    tool_call_id: &str,
) -> Result<bool, HandlerError> {
    let chat = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .map_err(|e| HandlerError::GetChat(e.to_string()))?;

    Ok(chat
        .messages
        .iter()
        .any(|m| m.tool_call_id.as_deref() == Some(tool_call_id)))
}

#[derive(Debug, thiserror::Error)]
pub enum HandlerError {
    #[error("failed to fetch chat: {0}")]
    GetChat(String),
    #[error("failed to add tool message: {0}")]
    AddMessage(String),
}
