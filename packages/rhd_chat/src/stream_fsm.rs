use std::sync::Arc;

use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::state::{ExecutionPhase, MessageQueue, QueuedMessage};

/// States of the stream lifecycle
#[derive(Debug)]
pub enum StreamLifecycleState {
    /// No active stream
    Idle,
    /// Stream is actively running
    Running {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
        phase: ExecutionPhase,
    },
    /// Stream is paused and waiting for resume/abort
    Paused {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
        phase: ExecutionPhase,
        message_queue: MessageQueue,
    },
    /// Stream has been aborted
    Aborted {
        cancel_token: CancellationToken,
        aborted_tool_ids: Vec<String>,
        message_queue: MessageQueue,
    },
}

/// Inputs that can be fed into the StreamLifecycleFsm
#[derive(Debug)]
pub enum StreamLifecycleInput {
    /// Register a new stream
    Register {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
    },
    /// Pause the stream
    Pause,
    /// Resume a paused stream
    Resume,
    /// Abort the stream
    Abort { aborted_tool_ids: Vec<String> },
    /// Set the current execution phase
    SetPhase { phase: ExecutionPhase },
    /// Queue a message for later processing
    QueueMessage { message: QueuedMessage },
    /// Unregister the stream (cleanup)
    Unregister,
}

/// Actions emitted by the StreamLifecycleFsm
#[derive(Debug)]
pub enum StreamLifecycleAction {
    /// Stream was registered
    Registered,
    /// Stream was paused
    Paused,
    /// Stream was resumed
    Resumed,
    /// Stream was aborted
    Aborted,
    /// Stream was unregistered
    Unregistered,
    /// Phase was changed
    PhaseChanged { phase: ExecutionPhase },
    /// Message was queued
    MessageQueued,
    /// Error occurred (invalid transition)
    Error { message: String },
}

impl std::fmt::Display for StreamLifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamLifecycleState::Idle => write!(f, "Idle"),
            StreamLifecycleState::Running { phase, .. } => {
                write!(f, "Running {{ phase: {:?} }}", phase)
            }
            StreamLifecycleState::Paused { phase, message_queue, .. } => {
                write!(f, "Paused {{ phase: {:?}, queue_size: {} }}",
                    phase, message_queue.len())
            }
            StreamLifecycleState::Aborted { aborted_tool_ids, message_queue, .. } => {
                write!(f, "Aborted {{ aborted_tools: {}, queue_size: {} }}",
                    aborted_tool_ids.len(), message_queue.len())
            }
        }
    }
}

impl std::fmt::Display for StreamLifecycleInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamLifecycleInput::Register { .. } => write!(f, "Register"),
            StreamLifecycleInput::Pause => write!(f, "Pause"),
            StreamLifecycleInput::Resume => write!(f, "Resume"),
            StreamLifecycleInput::Abort { aborted_tool_ids } => {
                write!(f, "Abort {{ aborted_tools: {} }}", aborted_tool_ids.len())
            }
            StreamLifecycleInput::SetPhase { phase } => {
                write!(f, "SetPhase {{ phase: {:?} }}", phase)
            }
            StreamLifecycleInput::QueueMessage { message } => {
                write!(f, "QueueMessage {{ content_length: {} }}", message.content.len())
            }
            StreamLifecycleInput::Unregister => write!(f, "Unregister"),
        }
    }
}

impl std::fmt::Display for StreamLifecycleAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamLifecycleAction::Registered => write!(f, "Registered"),
            StreamLifecycleAction::Paused => write!(f, "Paused"),
            StreamLifecycleAction::Resumed => write!(f, "Resumed"),
            StreamLifecycleAction::Aborted => write!(f, "Aborted"),
            StreamLifecycleAction::Unregistered => write!(f, "Unregistered"),
            StreamLifecycleAction::PhaseChanged { phase } => {
                write!(f, "PhaseChanged {{ phase: {:?} }}", phase)
            }
            StreamLifecycleAction::MessageQueued => write!(f, "MessageQueued"),
            StreamLifecycleAction::Error { message } => {
                write!(f, "Error {{ message: {} }}", message)
            }
        }
    }
}

/// Finite State Machine for managing stream lifecycle
pub struct StreamLifecycleFsm {
    chat_id: i64,
    state: StreamLifecycleState,
}

impl StreamLifecycleFsm {
    /// Create a new FSM in Idle state
    pub fn new(chat_id: i64) -> Self {
        Self {
            chat_id,
            state: StreamLifecycleState::Idle,
        }
    }

    /// Get the current state
    pub fn state(&self) -> &StreamLifecycleState {
        &self.state
    }

    /// Get the chat ID
    pub fn chat_id(&self) -> i64 {
        self.chat_id
    }

    /// Handle an input and return actions to execute
    pub fn handle(&mut self, input: StreamLifecycleInput) -> Vec<StreamLifecycleAction> {
        let mut actions = Vec::new();

        tracing::info!(
            fsm = "StreamLifecycleFsm",
            chat_id = self.chat_id,
            state_before = %self.state,
            input = %input,
            "StreamLifecycleFsm::handle started"
        );

        match (&self.state, input) {
            // Idle -> Register -> Running
            (
                StreamLifecycleState::Idle,
                StreamLifecycleInput::Register {
                    cancel_token,
                    pause_notify,
                },
            ) => {
                self.state = StreamLifecycleState::Running {
                    cancel_token,
                    pause_notify,
                    phase: ExecutionPhase::AiCall,
                };
                actions.push(StreamLifecycleAction::Registered);
            }

            // Running -> SetPhase -> Running
            (
                StreamLifecycleState::Running {
                    cancel_token,
                    pause_notify,
                    ..
                },
                StreamLifecycleInput::SetPhase { phase },
            ) => {
                self.state = StreamLifecycleState::Running {
                    cancel_token: cancel_token.clone(),
                    pause_notify: pause_notify.clone(),
                    phase: phase.clone(),
                };
                actions.push(StreamLifecycleAction::PhaseChanged { phase });
            }

            // Running -> Pause -> Paused
            (
                StreamLifecycleState::Running {
                    cancel_token,
                    pause_notify,
                    phase,
                },
                StreamLifecycleInput::Pause,
            ) => {
                self.state = StreamLifecycleState::Paused {
                    cancel_token: cancel_token.clone(),
                    pause_notify: pause_notify.clone(),
                    phase: phase.clone(),
                    message_queue: MessageQueue::new(),
                };
                actions.push(StreamLifecycleAction::Paused);
            }

            // Running -> Abort -> Aborted
            (
                StreamLifecycleState::Running { cancel_token, .. },
                StreamLifecycleInput::Abort { aborted_tool_ids },
            ) => {
                self.state = StreamLifecycleState::Aborted {
                    cancel_token: cancel_token.clone(),
                    aborted_tool_ids,
                    message_queue: MessageQueue::new(),
                };
                actions.push(StreamLifecycleAction::Aborted);
            }

            // Running -> Unregister -> Idle
            (StreamLifecycleState::Running { .. }, StreamLifecycleInput::Unregister) => {
                self.state = StreamLifecycleState::Idle;
                actions.push(StreamLifecycleAction::Unregistered);
            }

            // Paused -> Resume -> Running
            (
                StreamLifecycleState::Paused {
                    cancel_token,
                    pause_notify,
                    phase,
                    ..
                },
                StreamLifecycleInput::Resume,
            ) => {
                self.state = StreamLifecycleState::Running {
                    cancel_token: cancel_token.clone(),
                    pause_notify: pause_notify.clone(),
                    phase: phase.clone(),
                };
                actions.push(StreamLifecycleAction::Resumed);
            }

            // Paused -> Abort -> Aborted
            (
                StreamLifecycleState::Paused {
                    cancel_token,
                    message_queue,
                    ..
                },
                StreamLifecycleInput::Abort { aborted_tool_ids },
            ) => {
                self.state = StreamLifecycleState::Aborted {
                    cancel_token: cancel_token.clone(),
                    aborted_tool_ids,
                    message_queue: message_queue.clone(),
                };
                actions.push(StreamLifecycleAction::Aborted);
            }

            // Paused -> QueueMessage -> Paused
            (
                StreamLifecycleState::Paused {
                    cancel_token,
                    pause_notify,
                    phase,
                    message_queue,
                },
                StreamLifecycleInput::QueueMessage { message },
            ) => {
                let mut new_queue = message_queue.clone();
                new_queue.push(message);
                self.state = StreamLifecycleState::Paused {
                    cancel_token: cancel_token.clone(),
                    pause_notify: pause_notify.clone(),
                    phase: phase.clone(),
                    message_queue: new_queue,
                };
                actions.push(StreamLifecycleAction::MessageQueued);
            }

            // Paused -> Unregister -> Idle
            (StreamLifecycleState::Paused { .. }, StreamLifecycleInput::Unregister) => {
                self.state = StreamLifecycleState::Idle;
                actions.push(StreamLifecycleAction::Unregistered);
            }

            // Aborted -> QueueMessage -> Aborted
            (
                StreamLifecycleState::Aborted {
                    cancel_token,
                    aborted_tool_ids,
                    message_queue,
                },
                StreamLifecycleInput::QueueMessage { message },
            ) => {
                let mut new_queue = message_queue.clone();
                new_queue.push(message);
                self.state = StreamLifecycleState::Aborted {
                    cancel_token: cancel_token.clone(),
                    aborted_tool_ids: aborted_tool_ids.clone(),
                    message_queue: new_queue,
                };
                actions.push(StreamLifecycleAction::MessageQueued);
            }

            // Aborted -> Unregister -> Idle
            (StreamLifecycleState::Aborted { .. }, StreamLifecycleInput::Unregister) => {
                self.state = StreamLifecycleState::Idle;
                actions.push(StreamLifecycleAction::Unregistered);
            }

            // Invalid transitions
            (state, input) => {
                actions.push(StreamLifecycleAction::Error {
                    message: format!(
                        "Invalid transition: state={:?}, input={:?}",
                        std::mem::discriminant(state),
                        std::mem::discriminant(&input)
                    ),
                });
            }
        }

        for action in &actions {
            tracing::info!(
                fsm = "StreamLifecycleFsm",
                chat_id = self.chat_id,
                state_after = %self.state,
                action = %action,
                "Emitted action"
            );
        }

        tracing::info!(
            fsm = "StreamLifecycleFsm",
            chat_id = self.chat_id,
            final_state = %self.state,
            actions_count = actions.len(),
            "StreamLifecycleFsm::handle completed"
        );

        actions
    }

    /// Check if the stream is in Running state
    pub fn is_running(&self) -> bool {
        matches!(self.state, StreamLifecycleState::Running { .. })
    }

    /// Check if the stream is in Paused state
    pub fn is_paused(&self) -> bool {
        matches!(self.state, StreamLifecycleState::Paused { .. })
    }

    /// Check if the stream is in Aborted state
    pub fn is_aborted(&self) -> bool {
        matches!(self.state, StreamLifecycleState::Aborted { .. })
    }

    /// Check if the stream is in Idle state
    pub fn is_idle(&self) -> bool {
        matches!(self.state, StreamLifecycleState::Idle)
    }

    /// Get the cancel token if available
    pub fn cancel_token(&self) -> Option<&CancellationToken> {
        match &self.state {
            StreamLifecycleState::Running { cancel_token, .. } => Some(cancel_token),
            StreamLifecycleState::Paused { cancel_token, .. } => Some(cancel_token),
            StreamLifecycleState::Aborted { cancel_token, .. } => Some(cancel_token),
            StreamLifecycleState::Idle => None,
        }
    }

    /// Get the current phase if available
    pub fn phase(&self) -> Option<&ExecutionPhase> {
        match &self.state {
            StreamLifecycleState::Running { phase, .. } => Some(phase),
            StreamLifecycleState::Paused { phase, .. } => Some(phase),
            StreamLifecycleState::Aborted { .. } => None,
            StreamLifecycleState::Idle => None,
        }
    }

    /// Get the message queue if available
    pub fn message_queue(&self) -> Option<&MessageQueue> {
        match &self.state {
            StreamLifecycleState::Paused { message_queue, .. } => Some(message_queue),
            StreamLifecycleState::Aborted { message_queue, .. } => Some(message_queue),
            StreamLifecycleState::Running { .. } => None,
            StreamLifecycleState::Idle => None,
        }
    }

    /// Get mutable message queue if available
    pub fn message_queue_mut(&mut self) -> Option<&mut MessageQueue> {
        match &mut self.state {
            StreamLifecycleState::Paused { message_queue, .. } => Some(message_queue),
            StreamLifecycleState::Aborted { message_queue, .. } => Some(message_queue),
            StreamLifecycleState::Running { .. } => None,
            StreamLifecycleState::Idle => None,
        }
    }

    /// Get aborted tool IDs if in Aborted state
    pub fn aborted_tool_ids(&self) -> Option<&Vec<String>> {
        match &self.state {
            StreamLifecycleState::Aborted { aborted_tool_ids, .. } => Some(aborted_tool_ids),
            _ => None,
        }
    }

    /// Get the pause_notify if in Running or Paused state
    pub fn pause_notify(&self) -> Option<&Arc<Notify>> {
        match &self.state {
            StreamLifecycleState::Running { pause_notify, .. } => Some(pause_notify),
            StreamLifecycleState::Paused { pause_notify, .. } => Some(pause_notify),
            StreamLifecycleState::Aborted { .. } => None,
            StreamLifecycleState::Idle => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_fsm() -> StreamLifecycleFsm {
        StreamLifecycleFsm::new(1)
    }

    #[test]
    fn test_initial_state() {
        let fsm = create_test_fsm();
        assert!(fsm.is_idle());
    }

    #[test]
    fn test_register_from_idle() {
        let mut fsm = create_test_fsm();
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());

        let actions = fsm.handle(StreamLifecycleInput::Register {
            cancel_token,
            pause_notify,
        });

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            StreamLifecycleAction::Registered => {}
            _ => panic!("Expected Registered action"),
        }

        assert!(fsm.is_running());
    }

    #[test]
    fn test_pause_from_running() {
        let mut fsm = create_test_fsm();
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());

        fsm.handle(StreamLifecycleInput::Register {
            cancel_token,
            pause_notify,
        });

        let actions = fsm.handle(StreamLifecycleInput::Pause);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            StreamLifecycleAction::Paused => {}
            _ => panic!("Expected Paused action"),
        }

        assert!(fsm.is_paused());
    }

    #[test]
    fn test_resume_from_paused() {
        let mut fsm = create_test_fsm();
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());

        fsm.handle(StreamLifecycleInput::Register {
            cancel_token,
            pause_notify,
        });
        fsm.handle(StreamLifecycleInput::Pause);

        let actions = fsm.handle(StreamLifecycleInput::Resume);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            StreamLifecycleAction::Resumed => {}
            _ => panic!("Expected Resumed action"),
        }

        assert!(fsm.is_running());
    }

    #[test]
    fn test_abort_from_running() {
        let mut fsm = create_test_fsm();
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());

        fsm.handle(StreamLifecycleInput::Register {
            cancel_token,
            pause_notify,
        });

        let actions = fsm.handle(StreamLifecycleInput::Abort {
            aborted_tool_ids: vec!["call_1".to_string()],
        });

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            StreamLifecycleAction::Aborted => {}
            _ => panic!("Expected Aborted action"),
        }

        assert!(fsm.is_aborted());
    }

    #[test]
    fn test_abort_from_paused() {
        let mut fsm = create_test_fsm();
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());

        fsm.handle(StreamLifecycleInput::Register {
            cancel_token,
            pause_notify,
        });
        fsm.handle(StreamLifecycleInput::Pause);

        let actions = fsm.handle(StreamLifecycleInput::Abort {
            aborted_tool_ids: vec!["call_1".to_string()],
        });

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            StreamLifecycleAction::Aborted => {}
            _ => panic!("Expected Aborted action"),
        }

        assert!(fsm.is_aborted());
    }

    #[test]
    fn test_unregister_from_running() {
        let mut fsm = create_test_fsm();
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());

        fsm.handle(StreamLifecycleInput::Register {
            cancel_token,
            pause_notify,
        });

        let actions = fsm.handle(StreamLifecycleInput::Unregister);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            StreamLifecycleAction::Unregistered => {}
            _ => panic!("Expected Unregistered action"),
        }

        assert!(fsm.is_idle());
    }

    #[test]
    fn test_set_phase() {
        let mut fsm = create_test_fsm();
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());

        fsm.handle(StreamLifecycleInput::Register {
            cancel_token,
            pause_notify,
        });

        let actions = fsm.handle(StreamLifecycleInput::SetPhase {
            phase: ExecutionPhase::ToolExecution,
        });

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            StreamLifecycleAction::PhaseChanged { phase } => {
                assert_eq!(*phase, ExecutionPhase::ToolExecution);
            }
            _ => panic!("Expected PhaseChanged action"),
        }

        assert_eq!(fsm.phase(), Some(&ExecutionPhase::ToolExecution));
    }

    #[test]
    fn test_queue_message_while_paused() {
        let mut fsm = create_test_fsm();
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());

        fsm.handle(StreamLifecycleInput::Register {
            cancel_token,
            pause_notify,
        });
        fsm.handle(StreamLifecycleInput::Pause);

        let message = QueuedMessage {
            content: "test message".to_string(),
            model: "gpt-4".to_string(),
            queued_at: chrono::Utc::now(),
        };

        let actions = fsm.handle(StreamLifecycleInput::QueueMessage { message });

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            StreamLifecycleAction::MessageQueued => {}
            _ => panic!("Expected MessageQueued action"),
        }

        assert_eq!(fsm.message_queue().unwrap().len(), 1);
    }

    #[test]
    fn test_invalid_transition_register_from_running() {
        let mut fsm = create_test_fsm();
        let cancel_token = CancellationToken::new();
        let pause_notify = Arc::new(Notify::new());

        fsm.handle(StreamLifecycleInput::Register {
            cancel_token: cancel_token.clone(),
            pause_notify: pause_notify.clone(),
        });

        let actions = fsm.handle(StreamLifecycleInput::Register {
            cancel_token,
            pause_notify,
        });

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            StreamLifecycleAction::Error { message } => {
                assert!(message.contains("Invalid transition"));
            }
            _ => panic!("Expected Error action"),
        }
    }
}
