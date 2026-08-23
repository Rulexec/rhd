pub mod protocol;
pub mod transport;
pub mod client;
pub mod builtin;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;

pub static DEBUG: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    #[serde(skip)]
    pub raw_response: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub name: Option<String>,
    pub cmd: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("Transport error: {0}")]
    Transport(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type McpResult<T> = Result<T, McpError>;

pub trait McpClientTrait: Send + Sync {
    fn list_tools(&self) -> impl std::future::Future<Output = McpResult<Vec<ToolDefinition>>> + Send;
    fn call_tool(&self, name: &str, arguments: &str) -> impl std::future::Future<Output = McpResult<ToolResult>> + Send;
    fn has_tool(&self, name: &str) -> impl std::future::Future<Output = bool> + Send;
}
