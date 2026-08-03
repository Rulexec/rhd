use std::sync::Arc;

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

/// Tracks which phase of the tool loop is currently executing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionPhase {
    /// AI is being called (streaming response)
    AiCall,
    /// Tool calls are being executed
    ToolExecution,
}

/// Represents a tool call that was emitted by AI but not yet executed
#[derive(Debug, Clone, PartialEq)]
pub struct PendingToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// Represents a message queued while chat is paused/aborted
#[derive(Debug, Clone)]
pub struct QueuedMessage {
    pub content: String,
    pub model: String,
    pub queued_at: chrono::DateTime<chrono::Utc>,
}

/// Stores queued messages for a chat
#[derive(Debug, Clone, Default)]
pub struct MessageQueue {
    pub messages: Vec<QueuedMessage>,
}

impl MessageQueue {
    pub fn new() -> Self {
        Self { messages: Vec::new() }
    }

    pub fn push(&mut self, message: QueuedMessage) {
        self.messages.push(message);
    }

    pub fn drain(&mut self) -> Vec<QueuedMessage> {
        std::mem::take(&mut self.messages)
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }
}

#[derive(Debug)]
pub enum StreamState {
    Running {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
        phase: ExecutionPhase,
    },
    Paused {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
        phase: ExecutionPhase,
        message_queue: MessageQueue,
    },
    Aborted {
        cancel_token: CancellationToken,
        aborted_tool_ids: Vec<String>,
        message_queue: MessageQueue,
    },
}

impl StreamState {
    pub fn cancel_token(&self) -> &CancellationToken {
        match self {
            StreamState::Running { cancel_token, .. } => cancel_token,
            StreamState::Paused { cancel_token, .. } => cancel_token,
            StreamState::Aborted { cancel_token, .. } => cancel_token,
        }
    }

    pub fn is_aborted(&self) -> bool {
        matches!(self, StreamState::Aborted { .. })
    }

    pub fn phase(&self) -> Option<&ExecutionPhase> {
        match self {
            StreamState::Running { phase, .. } => Some(phase),
            StreamState::Paused { phase, .. } => Some(phase),
            StreamState::Aborted { .. } => None,
        }
    }


    pub fn message_queue(&self) -> Option<&MessageQueue> {
        match self {
            StreamState::Paused { message_queue, .. } => Some(message_queue),
            StreamState::Aborted { message_queue, .. } => Some(message_queue),
            StreamState::Running { .. } => None,
        }
    }

    pub fn message_queue_mut(&mut self) -> Option<&mut MessageQueue> {
        match self {
            StreamState::Paused { message_queue, .. } => Some(message_queue),
            StreamState::Aborted { message_queue, .. } => Some(message_queue),
            StreamState::Running { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamStateInfo {
    pub is_running: bool,
    pub is_paused: bool,
    pub is_aborted: bool,
    pub phase: Option<ExecutionPhase>,
    pub aborted_tool_ids: Vec<String>,
    pub queued_messages: Vec<QueuedMessage>,
}

impl From<&StreamState> for StreamStateInfo {
    fn from(state: &StreamState) -> Self {
        match state {
            StreamState::Running { phase, .. } => StreamStateInfo {
                is_running: true,
                is_paused: false,
                is_aborted: false,
                phase: Some(phase.clone()),
                aborted_tool_ids: Vec::new(),
                queued_messages: Vec::new(),
            },
            StreamState::Paused { phase, message_queue, .. } => StreamStateInfo {
                is_running: false,
                is_paused: true,
                is_aborted: false,
                phase: Some(phase.clone()),
                aborted_tool_ids: Vec::new(),
                queued_messages: message_queue.messages.clone(),
            },
            StreamState::Aborted { aborted_tool_ids, message_queue, .. } => StreamStateInfo {
                is_running: false,
                is_paused: false,
                is_aborted: true,
                phase: None,
                aborted_tool_ids: aborted_tool_ids.clone(),
                queued_messages: message_queue.messages.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_state_is_aborted() {
        let cancel_token = CancellationToken::new();
        let aborted = StreamState::Aborted {
            cancel_token,
            aborted_tool_ids: vec!["call_1".to_string()],
            message_queue: MessageQueue::new(),
        };
        assert!(aborted.is_aborted());

        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());
        let running = StreamState::Running {
            cancel_token,
            pause_notify,
            phase: ExecutionPhase::AiCall,
        };
        assert!(!running.is_aborted());
    }

    #[test]
    fn test_stream_state_phase() {
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());
        let running = StreamState::Running {
            cancel_token,
            pause_notify,
            phase: ExecutionPhase::ToolExecution,
        };
        assert_eq!(running.phase(), Some(&ExecutionPhase::ToolExecution));
    }


    #[test]
    fn test_message_queue_push_and_drain() {
        let mut queue = MessageQueue::new();
        assert!(queue.is_empty());
        
        queue.push(QueuedMessage {
            content: "Hello".to_string(),
            model: "gpt-4".to_string(),
            queued_at: chrono::Utc::now(),
        });
        
        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());
        
        let messages = queue.drain();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello");
        assert!(queue.is_empty());
    }

    #[test]
    fn test_message_queue_multiple_messages() {
        let mut queue = MessageQueue::new();
        
        queue.push(QueuedMessage {
            content: "First".to_string(),
            model: "gpt-4".to_string(),
            queued_at: chrono::Utc::now(),
        });
        
        queue.push(QueuedMessage {
            content: "Second".to_string(),
            model: "gpt-4".to_string(),
            queued_at: chrono::Utc::now(),
        });
        
        assert_eq!(queue.len(), 2);
        
        let messages = queue.drain();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "First");
        assert_eq!(messages[1].content, "Second");
    }
}
