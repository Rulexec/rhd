use rhd_db::Message;

#[derive(Debug, Clone)]
pub enum ChatEvent {
    StreamChunk { chat_id: i64, content: String },
    ThinkingChunk { chat_id: i64, content: String },
    StreamFinished { chat_id: i64, message_id: i64, finish_reason: String },
    StreamError { chat_id: i64, error: String },
    MessageAdded { chat_id: i64, message: Message },
    MessageRemoved { chat_id: i64, message_id: i64 },
    MessageReplaced { chat_id: i64, message: Message },
    DevNotification { title: String, message: String },
    ProjectAttached { chat_id: i64, project_name: String },
    ProjectDetached { chat_id: i64, project_name: String },
    ToolCallStarted {
        chat_id: i64,
        tool_call_id: String,
        tool_name: String,
        arguments: String,
        mcp_id: String,
    },
    ToolCallCompleted {
        chat_id: i64,
        tool_call_id: String,
        result: String,
        is_error: bool,
    },
    ChatPaused { chat_id: i64 },
    ChatResumed { chat_id: i64 },
    StreamAborted { chat_id: i64 },
    MessageQueued {
        chat_id: i64,
        content: String,
        model: String,
    },
    RoleChanged {
        chat_id: i64,
        project_name: String,
        role_name: String,
    },
    RolesUpdated { chat_id: i64 },
    ActiveRoleCleared { chat_id: i64 },
    TodoListUpdated {
        chat_id: i64,
        items: Vec<crate::TodoItem>,
    },
}
