//! Tool management handlers.

use serde_json::Value;

use rhd_chat_api::methods::{
    AddToolsParams, AddToolsResult, GetToolsParams, GetToolsResult, RemoveToolsParams,
    RemoveToolsResult,
};
use rhd_chat_api::protocol::Response;
use rhd_chat_api::tools::{FunctionDefinition, ToolDefinition as ApiToolDefinition, ToolInfo};
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;
use crate::events::tools_updated_event;
use crate::subscriptions::SharedSubscriptionManager;

/// Convert API ToolDefinition to DB ToolDefinition
fn api_to_db_tool(tool: &ApiToolDefinition) -> rhd_db::ToolDefinition {
    rhd_db::ToolDefinition {
        tool_type: tool.tool_type.clone(),
        function: rhd_db::FunctionDefinition {
            name: tool.function.name.clone(),
            description: tool.function.description.clone(),
            parameters: tool.function.parameters.clone(),
        },
    }
}

/// Convert DB ToolDefinition to API ToolDefinition
fn db_to_api_tool(tool: &rhd_db::ToolDefinition) -> ApiToolDefinition {
    ApiToolDefinition {
        tool_type: tool.tool_type.clone(),
        function: FunctionDefinition {
            name: tool.function.name.clone(),
            description: tool.function.description.clone(),
            parameters: tool.function.parameters.clone(),
        },
    }
}

/// Handle `addTools` request.
///
/// Adds tools to a chat. The tools are associated with the plugin that is making the request.
/// Returns an error if the plugin is not registered.
pub async fn add_tools(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    plugin_id: Option<&str>,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    // Validate that a plugin is registered for this connection
    let plugin_id = match plugin_id {
        Some(id) => id,
        None => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                "Plugin must be registered before adding tools".to_string(),
            ))?);
        }
    };

    let params: AddToolsParams = match serde_json::from_value(params) {
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

    // Add tools to the database
    // Convert API tools to DB tools
    let db_tools: Vec<rhd_db::ToolDefinition> = params.tools.iter().map(api_to_db_tool).collect();
    db.add_chat_tools(params.chat_id, plugin_id, &db_tools)?;

    // Touch chat to update updated_at and get new version
    let chat_version = db.touch_chat(params.chat_id)?;

    // Get all tools for the chat to broadcast
    let all_tools = db.get_chat_tools(params.chat_id)?;
    let tool_infos: Vec<ToolInfo> = all_tools
        .into_iter()
        .map(|(pid, tool)| ToolInfo {
            plugin_id: pid,
            tool: db_to_api_tool(&tool),
        })
        .collect();

    // Broadcast toolsUpdated event
    let event = tools_updated_event(params.chat_id, tool_infos, chat_version);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat(params.chat_id, event);

    let result = AddToolsResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `removeTools` request.
///
/// Removes tools from a chat by name. Only tools owned by the calling plugin can be removed.
/// Returns an error if the plugin is not registered.
pub async fn remove_tools(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    plugin_id: Option<&str>,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    // Validate that a plugin is registered for this connection
    let plugin_id = match plugin_id {
        Some(id) => id,
        None => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                "Plugin must be registered before removing tools".to_string(),
            ))?);
        }
    };

    let params: RemoveToolsParams = match serde_json::from_value(params) {
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

    // Remove tools from the database
    db.remove_chat_tools(params.chat_id, plugin_id, &params.tool_names)?;

    // Touch chat to update updated_at and get new version
    let chat_version = db.touch_chat(params.chat_id)?;

    // Get all tools for the chat to broadcast
    let all_tools = db.get_chat_tools(params.chat_id)?;
    let tool_infos: Vec<ToolInfo> = all_tools
        .into_iter()
        .map(|(pid, tool)| ToolInfo {
            plugin_id: pid,
            tool: db_to_api_tool(&tool),
        })
        .collect();

    // Broadcast toolsUpdated event
    let event = tools_updated_event(params.chat_id, tool_infos, chat_version);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat(params.chat_id, event);

    let result = RemoveToolsResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `getTools` request.
///
/// Gets all tools for a chat, including which plugin added each tool.
pub async fn get_tools(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Value, ServerError> {
    let params: GetToolsParams = match serde_json::from_value(params) {
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

    // Get all tools for the chat
    let all_tools = db.get_chat_tools(params.chat_id)?;
    let tool_infos: Vec<ToolInfo> = all_tools
        .into_iter()
        .map(|(pid, tool)| ToolInfo {
            plugin_id: pid,
            tool: db_to_api_tool(&tool),
        })
        .collect();

    let result = GetToolsResult { tools: tool_infos };
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}
