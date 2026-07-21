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
        pending_tool_calls: Vec<PendingToolCall>,
    },
    Aborted {
        cancel_token: CancellationToken,
        aborted_tool_ids: Vec<String>,
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

    pub fn pending_tool_calls(&self) -> Option<&Vec<PendingToolCall>> {
        match self {
            StreamState::Paused { pending_tool_calls, .. } => Some(pending_tool_calls),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamStateInfo {
    pub is_running: bool,
    pub is_paused: bool,
    pub is_aborted: bool,
    pub phase: Option<ExecutionPhase>,
    pub pending_tool_calls: Vec<PendingToolCall>,
    pub aborted_tool_ids: Vec<String>,
}

impl From<&StreamState> for StreamStateInfo {
    fn from(state: &StreamState) -> Self {
        match state {
            StreamState::Running { phase, .. } => StreamStateInfo {
                is_running: true,
                is_paused: false,
                is_aborted: false,
                phase: Some(phase.clone()),
                pending_tool_calls: Vec::new(),
                aborted_tool_ids: Vec::new(),
            },
            StreamState::Paused { phase, pending_tool_calls, .. } => StreamStateInfo {
                is_running: false,
                is_paused: true,
                is_aborted: false,
                phase: Some(phase.clone()),
                pending_tool_calls: pending_tool_calls.clone(),
                aborted_tool_ids: Vec::new(),
            },
            StreamState::Aborted { aborted_tool_ids, .. } => StreamStateInfo {
                is_running: false,
                is_paused: false,
                is_aborted: true,
                phase: None,
                pending_tool_calls: Vec::new(),
                aborted_tool_ids: aborted_tool_ids.clone(),
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
    fn test_stream_state_pending_tool_calls() {
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());
        let pending = vec![PendingToolCall {
            id: "call_1".to_string(),
            name: "test_tool".to_string(),
            arguments: "{}".to_string(),
        }];
        let paused = StreamState::Paused {
            cancel_token,
            pause_notify,
            phase: ExecutionPhase::AiCall,
            pending_tool_calls: pending.clone(),
        };
        assert_eq!(paused.pending_tool_calls(), Some(&pending));
    }
}
