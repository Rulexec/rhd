use serde::{Deserialize, Serialize};

// ============================================================================
// WebSocket protocol types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum WsRequest {
    #[serde(rename = "runScenario", rename_all = "camelCase")]
    RunScenario {
        id: String,
        name: String,
        cwd: String,
        #[serde(default)]
        model_aliases: Vec<(String, String)>,
    },
    #[serde(rename = "subscribe")]
    Subscribe { id: String },
    #[serde(rename = "getFinishedScenarios", rename_all = "camelCase")]
    GetFinishedScenarios { id: String, last_id: Option<u64> },
    #[serde(rename = "abortScenario")]
    AbortScenario { id: String, #[serde(rename = "executionId")] execution_id: u64 },

    // Chat operations
    #[serde(rename = "createChat")]
    CreateChat { id: String, title: String },
    #[serde(rename = "listChats")]
    ListChats { id: String },
    #[serde(rename = "getChat", rename_all = "camelCase")]
    GetChat { id: String, chat_id: i64 },
    #[serde(rename = "deleteChat", rename_all = "camelCase")]
    DeleteChat { id: String, chat_id: i64 },
    #[serde(rename = "deleteAllChats")]
    DeleteAllChats { id: String },
    #[serde(rename = "sendMessage", rename_all = "camelCase")]
    SendMessage {
        id: String,
        chat_id: i64,
        content: String,
        model: String,
    },
    #[serde(rename = "editMessage", rename_all = "camelCase")]
    EditMessage {
        id: String,
        message_id: i64,
        content: String,
        model: String,
    },
    #[serde(rename = "abortChat", rename_all = "camelCase")]
    AbortChat { id: String, chat_id: i64 },
    #[serde(rename = "getAvailableModels")]
    GetAvailableModels { id: String },

    // Project operations
    #[serde(rename = "listProjects")]
    ListProjects { id: String },
    #[serde(rename = "getProjectMcpStatus", rename_all = "camelCase")]
    GetProjectMcpStatus { id: String, project_name: String },

    // Chat-Project operations
    #[serde(rename = "attachProject", rename_all = "camelCase")]
    AttachProject { id: String, chat_id: i64, project_name: String },
    #[serde(rename = "detachProject", rename_all = "camelCase")]
    DetachProject { id: String, chat_id: i64, project_name: String },
    #[serde(rename = "getChatProjects", rename_all = "camelCase")]
    GetChatProjects { id: String, chat_id: i64 },

    // Chat pause/resume operations
    #[serde(rename = "pauseChat", rename_all = "camelCase")]
    PauseChat { id: String, chat_id: i64 },
    #[serde(rename = "resumeChat", rename_all = "camelCase")]
    ResumeChat { id: String, chat_id: i64 },

    // Scenario pause/resume operations
    #[serde(rename = "retryScenario", rename_all = "camelCase")]
    RetryScenario {
        id: String,
        execution_id: u64,
        model: Option<String>,
    },
    #[serde(rename = "abortScenarioWithError", rename_all = "camelCase")]
    AbortScenarioWithError { id: String, execution_id: u64 },
    #[serde(rename = "devNotification")]
    DevNotification { id: String },

    // Role operations
    #[serde(rename = "setRole", rename_all = "camelCase")]
    SetRole {
        id: String,
        chat_id: i64,
        project_name: String,
        role_name: String,
    },
    #[serde(rename = "getAvailableRoles", rename_all = "camelCase")]
    GetAvailableRoles { id: String, chat_id: i64 },
    #[serde(rename = "clearActiveRole", rename_all = "camelCase")]
    ClearActiveRole { id: String, chat_id: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsResponse {
    pub r#type: String,
    pub id: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<ErrorCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl WsResponse {
    pub fn success(id: String, data: serde_json::Value) -> Self {
        Self {
            r#type: "response".to_string(),
            id,
            success: true,
            data: Some(data),
            error_code: None,
            error: None,
        }
    }

    pub fn error(id: String, error_code: ErrorCode, error: String) -> Self {
        Self {
            r#type: "response".to_string(),
            id,
            success: false,
            data: None,
            error_code: Some(error_code),
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsEvent {
    pub r#type: String,
    pub event: String,
    pub data: serde_json::Value,
}

impl WsEvent {
    pub fn new(event: &str, data: serde_json::Value) -> Self {
        Self {
            r#type: "event".to_string(),
            event: event.to_string(),
            data,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    UnknownScenario,
    ScenarioExecutionFailed,
    ScenarioAborted,
    InvalidRequest,
    InternalError,
    ChatNotFound,
    MessageNotFound,
    ChatStreamFailed,
}
