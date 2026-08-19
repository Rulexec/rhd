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

impl std::fmt::Display for ToolLoopAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolLoopAction::SendToAi { messages, tools } => {
                write!(f, "SendToAi {{ messages: {}, tools: {} }}", messages.len(), tools.len())
            }
            ToolLoopAction::ExecuteToolCall { tool_call } => {
                write!(f, "ExecuteToolCall {{ name: {}, id: {} }}", tool_call.name, tool_call.id)
            }
            ToolLoopAction::Completed { message } => {
                write!(f, "Completed {{ message_id: {} }}", message.id)
            }
            ToolLoopAction::Paused => write!(f, "Paused"),
            ToolLoopAction::Aborted => write!(f, "Aborted"),
            ToolLoopAction::GenerateToolCallId { tool_call_id } => {
                write!(f, "GenerateToolCallId {{ id: {} }}", tool_call_id)
            }
            ToolLoopAction::Error { message } => {
                write!(f, "Error {{ message: {} }}", message)
            }
        }
    }
}
