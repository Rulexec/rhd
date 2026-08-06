use std::fmt;

/// Errors that can occur during FSM operation
#[derive(Debug, Clone, PartialEq)]
pub enum FsmError {
    /// Invalid state transition attempted
    InvalidTransition {
        from_state: String,
        input: String,
    },
    /// Input provided in wrong state
    UnexpectedInput {
        state: String,
        input: String,
    },
    /// Tool call result provided for unknown tool call
    UnknownToolCall {
        tool_call_id: String,
    },
    /// Message not found when trying to remove or replace
    MessageNotFound {
        message_id: i64,
    },
    /// Tool not found when trying to remove
    ToolNotFound {
        tool_name: String,
    },
    /// FSM is in terminal state and cannot process inputs
    FsmFinished {
        state: String,
    },
}

impl fmt::Display for FsmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsmError::InvalidTransition { from_state, input } => {
                write!(f, "Invalid transition from state '{}' with input '{}'", from_state, input)
            }
            FsmError::UnexpectedInput { state, input } => {
                write!(f, "Unexpected input '{}' in state '{}'", input, state)
            }
            FsmError::UnknownToolCall { tool_call_id } => {
                write!(f, "Unknown tool call ID: {}", tool_call_id)
            }
            FsmError::MessageNotFound { message_id } => {
                write!(f, "Message not found: {}", message_id)
            }
            FsmError::ToolNotFound { tool_name } => {
                write!(f, "Tool not found: {}", tool_name)
            }
            FsmError::FsmFinished { state } => {
                write!(f, "FSM is finished in state '{}'", state)
            }
        }
    }
}

impl std::error::Error for FsmError {}
