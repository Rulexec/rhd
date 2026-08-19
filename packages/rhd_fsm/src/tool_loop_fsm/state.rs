use serde::{Deserialize, Serialize};

/// Represents a tool call that was emitted by AI but not yet executed
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Represents a tool call result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}

/// Represents a chat message
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub thinking_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Represents a tool call from AI
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Represents a tool definition
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// States of the tool loop FSM
#[derive(Debug, Clone, PartialEq)]
pub enum State {
    /// Initial state, ready to start
    Idle,
    /// AI call in progress, waiting for response
    AwaitingAiResponse {
        /// Messages that were sent to AI
        sent_messages: Vec<ChatMessage>,
    },
    /// Tool calls received, waiting for results
    AwaitingToolResults {
        /// Pending tool calls that need results
        pending_tool_calls: Vec<PendingToolCall>,
        /// Collected tool results so far
        collected_results: Vec<ToolResult>,
    },
    /// Paused by user, can be resumed
    Paused {
        /// Pending tool calls if paused during tool execution
        pending_tool_calls: Vec<PendingToolCall>,
        /// Collected results before pause
        collected_results: Vec<ToolResult>,
    },
    /// Aborted, terminal state
    Aborted,
    /// Completed successfully, terminal state
    Completed {
        /// Final message ID
        message_id: i64,
    },
}

impl State {
    /// Returns true if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, State::Aborted | State::Completed { .. })
    }

    /// Returns the name of the state as a string
    pub fn name(&self) -> &'static str {
        match self {
            State::Idle => "Idle",
            State::AwaitingAiResponse { .. } => "AwaitingAiResponse",
            State::AwaitingToolResults { .. } => "AwaitingToolResults",
            State::Paused { .. } => "Paused",
            State::Aborted => "Aborted",
            State::Completed { .. } => "Completed",
        }
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            State::Idle => write!(f, "Idle"),
            State::AwaitingAiResponse { sent_messages } => {
                write!(f, "AwaitingAiResponse {{ sent_messages_count: {} }}", sent_messages.len())
            }
            State::AwaitingToolResults { pending_tool_calls, collected_results } => {
                write!(f, "AwaitingToolResults {{ pending: {}, collected: {} }}",
                    pending_tool_calls.len(), collected_results.len())
            }
            State::Paused { pending_tool_calls, collected_results } => {
                write!(f, "Paused {{ pending: {}, collected: {} }}",
                    pending_tool_calls.len(), collected_results.len())
            }
            State::Aborted => write!(f, "Aborted"),
            State::Completed { message_id } => {
                write!(f, "Completed {{ message_id: {} }}", message_id)
            }
        }
    }
}
