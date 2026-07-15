use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("database error: {0}")]
    Db(#[from] rhd_db::DbError),

    #[error("AI error: {0}")]
    Ai(#[from] rhd_ai::client::AiError),

    #[error("chat not found")]
    ChatNotFound,

    #[error("message not found")]
    MessageNotFound,

    #[error("model not found: {0}")]
    ModelNotFound(String),

    #[error("project not found: {0}")]
    ProjectNotFound(String),

    #[error("MCP server not connected for project: {0}")]
    McpNotConnected(String),

    #[error("MCP id conflict: {0}")]
    McpIdConflict(String),

    #[error("role name conflict: {0}")]
    RoleNameConflict(String),

    #[error("role not found: {0}")]
    RoleNotFound(String),

    #[error("internal error: {0}")]
    Internal(String),
}
