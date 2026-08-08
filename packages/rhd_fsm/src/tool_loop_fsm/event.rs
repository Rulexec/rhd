use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::state::{ChatMessage, State, ToolCall, ToolResult};

/// Events emitted by the ToolLoopFsm during state transitions and message mutations
#[derive(Debug, Clone)]
pub enum ToolLoopFsmEvent {
    /// A new message was inserted into the conversation
    MessageInserted {
        message: ChatMessage,
    },

    /// A message was removed from the conversation
    MessageRemoved {
        message_id: i64,
    },

    /// A message was replaced with a new one
    MessageReplaced {
        message_id: i64,
        new_message: ChatMessage,
    },

    /// All messages were replaced (bulk update)
    AllMessagesReplaced {
        messages: Vec<ChatMessage>,
    },

    /// A tool call was requested and is about to be executed
    /// Listeners can set `propagate` to false to intercept and handle the tool call themselves
    ToolCallRequested {
        tool_call: ToolCall,
        propagate: Arc<AtomicBool>,
    },

    /// A tool call was executed and produced a result
    ToolCallExecuted {
        tool_call: ToolCall,
        result: ToolResult,
    },

    /// AI response was received
    AiResponseReceived {
        content: Option<String>,
        thinking_content: Option<String>,
        tool_calls: Vec<ToolCall>,
    },

    /// A new tool call ID was generated
    ToolCallIdGenerated {
        tool_call_id: String,
    },

    /// FSM state changed
    StateChanged {
        from: State,
        to: State,
    },
}

impl ToolLoopFsmEvent {
    /// Returns the name of the event as a string
    pub fn name(&self) -> &'static str {
        match self {
            ToolLoopFsmEvent::MessageInserted { .. } => "MessageInserted",
            ToolLoopFsmEvent::MessageRemoved { .. } => "MessageRemoved",
            ToolLoopFsmEvent::MessageReplaced { .. } => "MessageReplaced",
            ToolLoopFsmEvent::AllMessagesReplaced { .. } => "AllMessagesReplaced",
            ToolLoopFsmEvent::ToolCallRequested { .. } => "ToolCallRequested",
            ToolLoopFsmEvent::ToolCallExecuted { .. } => "ToolCallExecuted",
            ToolLoopFsmEvent::AiResponseReceived { .. } => "AiResponseReceived",
            ToolLoopFsmEvent::ToolCallIdGenerated { .. } => "ToolCallIdGenerated",
            ToolLoopFsmEvent::StateChanged { .. } => "StateChanged",
        }
    }
}
