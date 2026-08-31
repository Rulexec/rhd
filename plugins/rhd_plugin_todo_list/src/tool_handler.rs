//! Tool call handler for rhd_set_todo_list.

use std::sync::Arc;

use rhd_chat_api::{AddMessageParams, AssistantMessageWithToolCallsData};
use rhd_chat_client::ChatClient;

use crate::parser;
use crate::templates::Templates;
use crate::todo_store::TodoStore;

/// Handle a tool call for rhd_set_todo_list.
///
/// This function:
/// 1. Checks if a tool result already exists for this tool_call_id
/// 2. Parses the arguments to extract the todos string
/// 3. Parses the markdown checklist
/// 4. If valid: stores the todo list and returns success
/// 5. If invalid: returns an error message with example
pub async fn handle_tool_call(
    client: Arc<ChatClient>,
    todo_store: Arc<TodoStore>,
    templates: Arc<Templates>,
    event: AssistantMessageWithToolCallsData,
) -> Result<(), HandlerError> {
    let chat_id = event.chat_id;
    let message = event.message;

    // Process each tool call in the message
    for tool_call in &message.tool_calls {
        if tool_call.function.name != "rhd_set_todo_list" {
            continue;
        }

        let tool_call_id = &tool_call.id;

        // Check if tool result already exists for this tool_call_id
        if has_tool_result(&client, chat_id, tool_call_id).await? {
            tracing::debug!(
                chat_id = chat_id,
                tool_call_id = %tool_call_id,
                "tool result already exists, skipping"
            );
            continue;
        }

        // Parse arguments
        let result = match parse_tool_arguments(&tool_call.function.arguments) {
            Ok(todos_string) => {
                // Parse the markdown checklist
                match parser::parse_todo_list(&todos_string) {
                    Ok(items) => {
                        // Store the todo list
                        todo_store.set(chat_id, items.clone()).await;
                        tracing::info!(
                            chat_id = chat_id,
                            items_count = items.len(),
                            "todo list updated"
                        );
                        ToolResult::Success("Todo list updated successfully.".to_string())
                    }
                    Err(e) => {
                        tracing::warn!(
                            chat_id = chat_id,
                            error = %e,
                            "failed to parse todo list"
                        );
                        ToolResult::Error(templates.tool_error_invalid_format().to_string())
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    chat_id = chat_id,
                    error = %e,
                    "failed to parse tool arguments"
                );
                ToolResult::Error(templates.tool_error_invalid_format().to_string())
            }
        };

        // Add tool result message
        let content = match result {
            ToolResult::Success(msg) => msg,
            ToolResult::Error(msg) => msg,
        };

        client
            .add_message(AddMessageParams {
                chat_id,
                role: "tool".to_string(),
                content,
                tool_call_id: Some(tool_call_id.clone()),
                reasoning_content: None,
                tags: vec![],
                is_finished: true,
                is_streaming: false,
            })
            .await
            .map_err(|e| HandlerError::AddMessage(e.to_string()))?;
    }

    Ok(())
}

/// Result of processing a tool call.
enum ToolResult {
    Success(String),
    Error(String),
}

/// Parse tool call arguments to extract the todos string.
fn parse_tool_arguments(arguments: &str) -> Result<String, HandlerError> {
    let json: serde_json::Value =
        serde_json::from_str(arguments).map_err(|e| HandlerError::InvalidArguments(e.to_string()))?;

    json.get("todos")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| HandlerError::InvalidArguments("missing 'todos' field".to_string()))
}

/// Check if a tool result message already exists for the given tool_call_id.
async fn has_tool_result(
    client: &ChatClient,
    chat_id: i64,
    tool_call_id: &str,
) -> Result<bool, HandlerError> {
    let chat = client
        .get_chat(rhd_chat_api::GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .map_err(|e| HandlerError::GetChat(e.to_string()))?;

    // Check if any message has this tool_call_id
    let has_result = chat
        .messages
        .iter()
        .any(|m| m.tool_call_id.as_deref() == Some(tool_call_id));

    Ok(has_result)
}

/// Errors that can occur during tool handling.
#[derive(Debug, thiserror::Error)]
pub enum HandlerError {
    #[error("invalid arguments: {0}")]
    InvalidArguments(String),
    #[error("failed to get chat: {0}")]
    GetChat(String),
    #[error("failed to add message: {0}")]
    AddMessage(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_arguments_valid() {
        let args = r#"{"todos": "[ ] Task 1\n[x] Task 2"}"#;
        let result = parse_tool_arguments(args).unwrap();
        assert_eq!(result, "[ ] Task 1\n[x] Task 2");
    }

    #[test]
    fn test_parse_tool_arguments_missing_todos() {
        let args = r#"{"other": "value"}"#;
        let result = parse_tool_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_tool_arguments_invalid_json() {
        let args = "not json";
        let result = parse_tool_arguments(args);
        assert!(result.is_err());
    }
}
