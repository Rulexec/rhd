use super::state::{ChatMessage, ToolCall, ToolDefinition};

/// Actions emitted by the ToolLoopFsm
#[derive(Debug, Clone)]
pub enum ToolLoopAction {
    /// Send messages to AI and wait for response
    SendToAi {
        /// Messages to send
        messages: Vec<ChatMessage>,
        /// Available tools
        tools: Vec<ToolDefinition>,
    },

    /// Execute a tool call
    ExecuteToolCall {
        /// Tool call to execute
        tool_call: ToolCall,
    },

    /// Tool loop completed successfully
    Completed {
        /// Final assistant message
        message: ChatMessage,
    },

    /// Tool loop paused
    Paused,

    /// Tool loop aborted
    Aborted,

    /// Request a new unique tool call ID
    GenerateToolCallId {
        /// The generated tool call ID
        tool_call_id: String,
    },

    /// Error occurred
    Error {
        message: String,
    },
}

impl ToolLoopAction {
    /// Returns the name of the action as a string
    pub fn name(&self) -> &'static str {
        match self {
            ToolLoopAction::SendToAi { .. } => "SendToAi",
            ToolLoopAction::ExecuteToolCall { .. } => "ExecuteToolCall",
            ToolLoopAction::Completed { .. } => "Completed",
            ToolLoopAction::Paused => "Paused",
            ToolLoopAction::Aborted => "Aborted",
            ToolLoopAction::GenerateToolCallId { .. } => "GenerateToolCallId",
            ToolLoopAction::Error { .. } => "Error",
        }
    }
}
