use chrono::Utc;
use rusqlite::{params, Connection};
use std::sync::Mutex;

use crate::{DbError, DbResult};

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

/// Tool definition structure (copied from rhd_ai)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Add tools for a chat (associated with a plugin)
pub(crate) fn add_chat_tools(
    conn: &Mutex<Connection>,
    chat_id: i64,
    plugin_id: &str,
    tools: &[ToolDefinition],
) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn.unchecked_transaction()?;
    let now = now_iso();

    for tool in tools {
        let tool_json = serde_json::to_string(tool)
            .map_err(|e| DbError::InitializationError(format!("Failed to serialize tool: {}", e)))?;
        tx.execute(
            "INSERT OR REPLACE INTO chat_tools (chat_id, plugin_id, tool_name, tool_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![chat_id, plugin_id, tool.function.name, tool_json, now],
        )?;
    }

    tx.execute(
        "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
        params![now, chat_id],
    )?;
    tx.commit()?;
    Ok(())
}

/// Remove tools by name for a chat (only if owned by the plugin)
pub(crate) fn remove_chat_tools(
    conn: &Mutex<Connection>,
    chat_id: i64,
    plugin_id: &str,
    tool_names: &[String],
) -> DbResult<()> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn.unchecked_transaction()?;

    for tool_name in tool_names {
        tx.execute(
            "DELETE FROM chat_tools WHERE chat_id = ?1 AND plugin_id = ?2 AND tool_name = ?3",
            params![chat_id, plugin_id, tool_name],
        )?;
    }

    let now = now_iso();
    tx.execute(
        "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
        params![now, chat_id],
    )?;
    tx.commit()?;
    Ok(())
}

/// Get all tools for a chat, returning (plugin_id, tool) pairs
pub(crate) fn get_chat_tools(
    conn: &Mutex<Connection>,
    chat_id: i64,
) -> DbResult<Vec<(String, ToolDefinition)>> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT plugin_id, tool_json FROM chat_tools WHERE chat_id = ?1 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![chat_id], |row| {
        let plugin_id: String = row.get(0)?;
        let tool_json: String = row.get(1)?;
        let tool: ToolDefinition = serde_json::from_str(&tool_json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
        Ok((plugin_id, tool))
    })?;
    let mut tools = Vec::new();
    for row in rows {
        tools.push(row?);
    }
    Ok(tools)
}

/// Get tools for a chat filtered by plugin_id
pub(crate) fn get_chat_tools_by_plugin(
    conn: &Mutex<Connection>,
    chat_id: i64,
    plugin_id: &str,
) -> DbResult<Vec<ToolDefinition>> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT tool_json FROM chat_tools WHERE chat_id = ?1 AND plugin_id = ?2 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![chat_id, plugin_id], |row| {
        let tool_json: String = row.get(0)?;
        let tool: ToolDefinition = serde_json::from_str(&tool_json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
        Ok(tool)
    })?;
    let mut tools = Vec::new();
    for row in rows {
        tools.push(row?);
    }
    Ok(tools)
}
