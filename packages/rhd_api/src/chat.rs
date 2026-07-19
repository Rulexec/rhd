use serde::{Deserialize, Serialize};

// ============================================================================
// Chat event payload types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamChunkEvent {
    pub chat_id: i64,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatThinkingChunkEvent {
    pub chat_id: i64,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamFinishedEvent {
    pub chat_id: i64,
    pub message_id: i64,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamErrorEvent {
    pub chat_id: i64,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageAddedEvent {
    pub chat_id: i64,
    pub message: ChatMessageDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageDto {
    pub id: i64,
    pub chat_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub model: Option<String>,
    pub thinking_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatUpdatedEvent {
    pub chat_id: i64,
    pub title: String,
}

// ============================================================================
// Project event types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMcpStatusChangedEvent {
    pub project_name: String,
    pub mcp_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAttachedEvent {
    pub chat_id: i64,
    pub project_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetachedEvent {
    pub chat_id: i64,
    pub project_name: String,
}

// ============================================================================
// Tool call event types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallStartedEvent {
    pub chat_id: i64,
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: String,
    pub mcp_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallCompletedEvent {
    pub chat_id: i64,
    pub tool_call_id: String,
    pub result: String,
    pub is_error: bool,
}

// ============================================================================
// Chat pause/resume event types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPausedEvent {
    pub chat_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResumedEvent {
    pub chat_id: i64,
}

// ============================================================================
// Scenario pause/resume event types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioPausedEvent {
    pub execution_id: u64,
    pub error: String,
    pub step_name: String,
    pub available_models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioResumedEvent {
    pub execution_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevNotificationEvent {
    pub title: String,
    pub message: String,
}

// ============================================================================
// Role event types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleInfo {
    pub project_name: String,
    pub role_name: String,
    pub when_to_use: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleChangedEvent {
    pub chat_id: i64,
    pub project_name: String,
    pub role_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RolesUpdatedEvent {
    pub chat_id: i64,
    pub roles: Vec<RoleInfo>,
    pub active_role_project: Option<String>,
    pub active_role_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRoleClearedEvent {
    pub chat_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItemDto {
    pub content: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoListUpdatedEvent {
    pub chat_id: i64,
    pub items: Vec<TodoItemDto>,
}
