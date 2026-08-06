use super::state::{ChatMessage, ToolCall, ToolDefinition, ToolResult};

/// Inputs that can be fed into the ToolLoopFsm
#[derive(Debug, Clone)]
pub enum ToolLoopInput {
    /// Start the tool loop (from Idle state)
    Run,

    /// Insert a chat message into the conversation
    InsertMessage { message: ChatMessage },

    /// Remove a chat message by ID
    RemoveMessage { message_id: i64 },

    /// Replace a chat message with a new one
    ReplaceMessage { message_id: i64, new_message: ChatMessage },

    /// Replace all messages with a new set
    ReplaceAllMessages { messages: Vec<ChatMessage> },

    /// Add an available tool definition
    AddTool { tool: ToolDefinition },

    /// Remove an available tool by name
    RemoveTool { tool_name: String },

    /// Provide AI response after a SendToAi action
    ProvideAiResponse {
        /// Content from AI
        content: Option<String>,
        /// Thinking/reasoning content
        thinking_content: Option<String>,
        /// Tool calls from AI (empty if final response)
        tool_calls: Vec<ToolCall>,
        /// Finish reason
        finish_reason: String,
    },

    /// Request next tool call ID (returns GenerateToolCallId action with the ID)
    RequestToolCallId,

    /// Provide tool call result
    ProvideToolResult {
        tool_call_id: String,
        result: ToolResult,
    },

    /// Pause the tool loop
    Pause,

    /// Abort the tool loop
    Abort,

    /// Resume from paused state
    Resume,
}

impl ToolLoopInput {
    /// Returns the name of the input as a string
    pub fn name(&self) -> &'static str {
        match self {
            ToolLoopInput::Run => "Run",
            ToolLoopInput::InsertMessage { .. } => "InsertMessage",
            ToolLoopInput::RemoveMessage { .. } => "RemoveMessage",
            ToolLoopInput::ReplaceMessage { .. } => "ReplaceMessage",
            ToolLoopInput::ReplaceAllMessages { .. } => "ReplaceAllMessages",
            ToolLoopInput::AddTool { .. } => "AddTool",
            ToolLoopInput::RemoveTool { .. } => "RemoveTool",
            ToolLoopInput::ProvideAiResponse { .. } => "ProvideAiResponse",
            ToolLoopInput::RequestToolCallId => "RequestToolCallId",
            ToolLoopInput::ProvideToolResult { .. } => "ProvideToolResult",
            ToolLoopInput::Pause => "Pause",
            ToolLoopInput::Abort => "Abort",
            ToolLoopInput::Resume => "Resume",
        }
    }
}
