use chrono::{DateTime, Utc};
use rhd_api::{LogSection, StepTiming, StepType, TokenUsage};
use tokio::sync::{oneshot, watch};

use super::{AbortHandle, PausedState, ResumeAction};

/// States of the execution lifecycle
#[derive(Debug, Clone)]
pub enum ExecutionState {
    /// Execution has not started yet
    Idle,
    /// Execution is actively running
    Running {
        scenario_name: String,
        started_at: DateTime<Utc>,
        step_timings: Vec<StepTiming>,
        token_usage: TokenUsage,
        current_step: Option<CurrentStep>,
    },
    /// Execution is paused and waiting for resume/abort
    Paused {
        scenario_name: String,
        started_at: DateTime<Utc>,
        pause_reason: String,
        step_name: String,
        step_timings: Vec<StepTiming>,
        token_usage: TokenUsage,
    },
    /// Execution has been aborted
    Aborted,
    /// Execution completed successfully
    Finished,
}

/// Current step information being executed
#[derive(Debug, Clone)]
pub struct CurrentStep {
    pub name: String,
    pub step_type: StepType,
    pub started_at: DateTime<Utc>,
    pub exit_code: Option<i32>,
    pub model: Option<String>,
    pub tokens: TokenUsage,
    pub sections: Vec<LogSection>,
}

/// Inputs that can be fed into the ExecutionFsm
#[derive(Debug, Clone)]
pub enum ExecutionInput {
    /// Start a new execution
    Start {
        scenario_name: String,
        started_at: DateTime<Utc>,
    },
    /// Pause the execution due to an error
    Pause {
        error: String,
        step_name: String,
    },
    /// Resume a paused execution
    Resume {
        model_override: Option<String>,
    },
    /// Abort the execution
    Abort,
    /// Mark the start of a step
    StepStarted {
        step_name: String,
        step_type: StepType,
        started_at: DateTime<Utc>,
    },
    /// Add token usage to current execution
    AddTokenUsage {
        usage: TokenUsage,
    },
    /// Add a log section to current step
    AddSection {
        section: LogSection,
    },
    /// Set exit code for current step
    SetStepExitCode {
        exit_code: i32,
    },
    /// Set model for current step
    SetStepModel {
        model: String,
    },
    /// Mark execution as finished
    Finish {
        finished_at: DateTime<Utc>,
    },
}

/// Actions emitted by the ExecutionFsm
#[derive(Debug)]
pub enum ExecutionAction {
    /// Emit scenario started event
    EmitStarted {
        id: u64,
        scenario_name: String,
        started_at: DateTime<Utc>,
    },
    /// Emit scenario paused event
    EmitPaused {
        id: u64,
        error: String,
        step_name: String,
    },
    /// Emit scenario resumed event
    EmitResumed {
        id: u64,
    },
    /// Emit scenario aborted event
    EmitAborted {
        id: u64,
    },
    /// Emit scenario finished event
    EmitFinished {
        id: u64,
        scenario_name: String,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
        step_timings: Vec<StepTiming>,
        token_usage: Option<TokenUsage>,
    },
    /// Emit step started event
    EmitStepStarted {
        id: u64,
        step_name: String,
        started_at: DateTime<Utc>,
    },
    /// Error occurred (invalid transition)
    Error {
        message: String,
    },
}

/// Finite State Machine for managing execution lifecycle
pub struct ExecutionFsm {
    id: u64,
    state: ExecutionState,
    abort_handle: AbortHandle,
    resume_tx: Option<oneshot::Sender<ResumeAction>>,
}

impl ExecutionFsm {
    /// Create a new FSM in Idle state
    pub fn new(id: u64) -> Self {
        Self {
            id,
            state: ExecutionState::Idle,
            abort_handle: AbortHandle::new(),
            resume_tx: None,
        }
    }

    /// Get the current state
    pub fn state(&self) -> &ExecutionState {
        &self.state
    }

    /// Get the abort receiver
    pub fn abort_receiver(&self) -> watch::Receiver<bool> {
        self.abort_handle.receiver()
    }

    /// Handle an input and return actions to execute
    pub fn handle(&mut self, input: ExecutionInput) -> Vec<ExecutionAction> {
        let old_state = self.state.clone();
        let mut actions = Vec::new();

        tracing::info!(
            execution_id = self.id,
            input = ?std::mem::discriminant(&input),
            current_state = ?std::mem::discriminant(&self.state),
            "ExecutionFsm::handle"
        );

        match (&self.state, input) {
            // Idle -> Start -> Running
            (ExecutionState::Idle, ExecutionInput::Start { scenario_name, started_at }) => {
                self.state = ExecutionState::Running {
                    scenario_name: scenario_name.clone(),
                    started_at,
                    step_timings: Vec::new(),
                    token_usage: TokenUsage::default(),
                    current_step: None,
                };
                actions.push(ExecutionAction::EmitStarted {
                    id: self.id,
                    scenario_name,
                    started_at,
                });
            }

            // Running -> StepStarted -> Running (with current step)
            (
                ExecutionState::Running {
                    scenario_name,
                    started_at,
                    step_timings,
                    token_usage,
                    ..
                },
                ExecutionInput::StepStarted {
                    step_name,
                    step_type,
                    started_at: step_started_at,
                },
            ) => {
                // Finalize previous step if exists
                let mut new_step_timings = step_timings.clone();
                if let Some(current_step) = self.get_current_step() {
                    let timing = Self::finalize_step(current_step, step_started_at);
                    new_step_timings.push(timing);
                }

                self.state = ExecutionState::Running {
                    scenario_name: scenario_name.clone(),
                    started_at: *started_at,
                    step_timings: new_step_timings,
                    token_usage: token_usage.clone(),
                    current_step: Some(CurrentStep {
                        name: step_name.clone(),
                        step_type,
                        started_at: step_started_at,
                        exit_code: None,
                        model: None,
                        tokens: TokenUsage::default(),
                        sections: Vec::new(),
                    }),
                };
                actions.push(ExecutionAction::EmitStepStarted {
                    id: self.id,
                    step_name,
                    started_at: step_started_at,
                });
            }

            // Running -> AddTokenUsage -> Running
            (
                ExecutionState::Running {
                    scenario_name,
                    started_at,
                    step_timings,
                    token_usage,
                    current_step,
                },
                ExecutionInput::AddTokenUsage { usage },
            ) => {
                let mut new_token_usage = token_usage.clone();
                new_token_usage.add(&usage);

                let mut new_current_step = current_step.clone();
                if let Some(ref mut step) = new_current_step {
                    step.tokens.add(&usage);
                }

                self.state = ExecutionState::Running {
                    scenario_name: scenario_name.clone(),
                    started_at: *started_at,
                    step_timings: step_timings.clone(),
                    token_usage: new_token_usage,
                    current_step: new_current_step,
                };
            }

            // Running -> AddSection -> Running
            (
                ExecutionState::Running {
                    scenario_name,
                    started_at,
                    step_timings,
                    token_usage,
                    current_step,
                },
                ExecutionInput::AddSection { section },
            ) => {
                let mut new_current_step = current_step.clone();
                if let Some(ref mut step) = new_current_step {
                    step.sections.push(section);
                }

                self.state = ExecutionState::Running {
                    scenario_name: scenario_name.clone(),
                    started_at: *started_at,
                    step_timings: step_timings.clone(),
                    token_usage: token_usage.clone(),
                    current_step: new_current_step,
                };
            }

            // Running -> SetStepExitCode -> Running
            (
                ExecutionState::Running {
                    scenario_name,
                    started_at,
                    step_timings,
                    token_usage,
                    current_step,
                },
                ExecutionInput::SetStepExitCode { exit_code },
            ) => {
                let mut new_current_step = current_step.clone();
                if let Some(ref mut step) = new_current_step {
                    step.exit_code = Some(exit_code);
                }

                self.state = ExecutionState::Running {
                    scenario_name: scenario_name.clone(),
                    started_at: *started_at,
                    step_timings: step_timings.clone(),
                    token_usage: token_usage.clone(),
                    current_step: new_current_step,
                };
            }

            // Running -> SetStepModel -> Running
            (
                ExecutionState::Running {
                    scenario_name,
                    started_at,
                    step_timings,
                    token_usage,
                    current_step,
                },
                ExecutionInput::SetStepModel { model },
            ) => {
                let mut new_current_step = current_step.clone();
                if let Some(ref mut step) = new_current_step {
                    step.model = Some(model);
                }

                self.state = ExecutionState::Running {
                    scenario_name: scenario_name.clone(),
                    started_at: *started_at,
                    step_timings: step_timings.clone(),
                    token_usage: token_usage.clone(),
                    current_step: new_current_step,
                };
            }

            // Running -> Pause -> Paused
            (
                ExecutionState::Running {
                    scenario_name,
                    started_at,
                    step_timings,
                    token_usage,
                    ..
                },
                ExecutionInput::Pause { error, step_name },
            ) => {
                let (tx, _rx) = oneshot::channel();
                self.resume_tx = Some(tx);

                self.state = ExecutionState::Paused {
                    scenario_name: scenario_name.clone(),
                    started_at: *started_at,
                    pause_reason: error.clone(),
                    step_name: step_name.clone(),
                    step_timings: step_timings.clone(),
                    token_usage: token_usage.clone(),
                };
                actions.push(ExecutionAction::EmitPaused {
                    id: self.id,
                    error,
                    step_name,
                });
                // Note: The resume channel is created internally, tracker can access it via take_resume_tx()
            }

            // Running -> Abort -> Aborted
            (
                ExecutionState::Running { .. },
                ExecutionInput::Abort,
            ) => {
                self.abort_handle.abort();
                self.state = ExecutionState::Aborted;
                actions.push(ExecutionAction::EmitAborted { id: self.id });
            }

            // Running -> Finish -> Finished
            (
                ExecutionState::Running {
                    scenario_name,
                    started_at,
                    step_timings,
                    token_usage,
                    current_step,
                },
                ExecutionInput::Finish { finished_at },
            ) => {
                let mut final_step_timings = step_timings.clone();
                if let Some(current) = current_step {
                    let timing = Self::finalize_step(current.clone(), finished_at);
                    final_step_timings.push(timing);
                }

                let tokens = if token_usage.total_tokens > 0 {
                    Some(token_usage.clone())
                } else {
                    None
                };

                let scenario_name_clone = scenario_name.clone();
                let started_at_clone = *started_at;
                
                self.state = ExecutionState::Finished;
                actions.push(ExecutionAction::EmitFinished {
                    id: self.id,
                    scenario_name: scenario_name_clone,
                    started_at: started_at_clone,
                    finished_at,
                    step_timings: final_step_timings,
                    token_usage: tokens,
                });
            }

            // Paused -> Resume -> Running
            (
                ExecutionState::Paused {
                    scenario_name,
                    started_at,
                    step_timings,
                    token_usage,
                    ..
                },
                ExecutionInput::Resume { model_override: _ },
            ) => {
                self.state = ExecutionState::Running {
                    scenario_name: scenario_name.clone(),
                    started_at: *started_at,
                    step_timings: step_timings.clone(),
                    token_usage: token_usage.clone(),
                    current_step: None,
                };
                self.resume_tx = None;
                actions.push(ExecutionAction::EmitResumed { id: self.id });
            }

            // Paused -> Abort -> Aborted
            (
                ExecutionState::Paused { .. },
                ExecutionInput::Abort,
            ) => {
                self.abort_handle.abort();
                self.state = ExecutionState::Aborted;
                actions.push(ExecutionAction::EmitAborted { id: self.id });
            }

            // Invalid transitions
            (state, input) => {
                actions.push(ExecutionAction::Error {
                    message: format!(
                        "Invalid transition: state={:?}, input={:?}",
                        std::mem::discriminant(state),
                        std::mem::discriminant(&input)
                    ),
                });
            }
        }

        // Log state transition
        tracing::info!(
            execution_id = self.id,
            from_state = ?std::mem::discriminant(&old_state),
            to_state = ?std::mem::discriminant(&self.state),
            "ExecutionFsm state transition"
        );

        actions
    }

    /// Get the resume sender if in Paused state
    pub fn take_resume_tx(&mut self) -> Option<oneshot::Sender<ResumeAction>> {
        self.resume_tx.take()
    }

    /// Get current step if in Running state
    fn get_current_step(&self) -> Option<CurrentStep> {
        if let ExecutionState::Running { current_step, .. } = &self.state {
            current_step.clone()
        } else {
            None
        }
    }

    /// Finalize a step and create a StepTiming
    fn finalize_step(step: CurrentStep, finished_at: DateTime<Utc>) -> StepTiming {
        StepTiming {
            name: step.name,
            step_type: step.step_type,
            exit_code: step.exit_code,
            model: step.model,
            started_at: step.started_at,
            finished_at,
            duration_ms: (finished_at - step.started_at).num_milliseconds() as u64,
            tokens: if step.tokens.total_tokens > 0 {
                Some(step.tokens)
            } else {
                None
            },
            cost: None,
            sections: step.sections,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_fsm() -> ExecutionFsm {
        ExecutionFsm::new(1)
    }

    #[test]
    fn test_initial_state() {
        let fsm = create_test_fsm();
        match fsm.state() {
            ExecutionState::Idle => {}
            _ => panic!("Expected Idle state"),
        }
    }

    #[test]
    fn test_start_from_idle() {
        let mut fsm = create_test_fsm();
        let started_at = Utc::now();
        let actions = fsm.handle(ExecutionInput::Start {
            scenario_name: "test_scenario".to_string(),
            started_at,
        });

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            ExecutionAction::EmitStarted { id, scenario_name, started_at: ts } => {
                assert_eq!(*id, 1);
                assert_eq!(scenario_name, "test_scenario");
                assert_eq!(*ts, started_at);
            }
            _ => panic!("Expected EmitStarted action"),
        }

        match fsm.state() {
            ExecutionState::Running { scenario_name, .. } => {
                assert_eq!(scenario_name, "test_scenario");
            }
            _ => panic!("Expected Running state"),
        }
    }

    #[test]
    fn test_step_started() {
        let mut fsm = create_test_fsm();
        let started_at = Utc::now();
        fsm.handle(ExecutionInput::Start {
            scenario_name: "test".to_string(),
            started_at,
        });

        let step_started_at = Utc::now();
        let actions = fsm.handle(ExecutionInput::StepStarted {
            step_name: "step1".to_string(),
            step_type: StepType::RunCommand,
            started_at: step_started_at,
        });

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            ExecutionAction::EmitStepStarted { id, step_name, .. } => {
                assert_eq!(*id, 1);
                assert_eq!(step_name, "step1");
            }
            _ => panic!("Expected EmitStepStarted action"),
        }
    }

    #[test]
    fn test_pause_from_running() {
        let mut fsm = create_test_fsm();
        let started_at = Utc::now();
        fsm.handle(ExecutionInput::Start {
            scenario_name: "test".to_string(),
            started_at,
        });

        let actions = fsm.handle(ExecutionInput::Pause {
            error: "test error".to_string(),
            step_name: "step1".to_string(),
        });

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            ExecutionAction::EmitPaused { id, error, step_name } => {
                assert_eq!(*id, 1);
                assert_eq!(error, "test error");
                assert_eq!(step_name, "step1");
            }
            _ => panic!("Expected EmitPaused action"),
        }

        match fsm.state() {
            ExecutionState::Paused { pause_reason, step_name, .. } => {
                assert_eq!(pause_reason, "test error");
                assert_eq!(step_name, "step1");
            }
            _ => panic!("Expected Paused state"),
        }
    }

    #[test]
    fn test_resume_from_paused() {
        let mut fsm = create_test_fsm();
        let started_at = Utc::now();
        fsm.handle(ExecutionInput::Start {
            scenario_name: "test".to_string(),
            started_at,
        });
        fsm.handle(ExecutionInput::Pause {
            error: "error".to_string(),
            step_name: "step1".to_string(),
        });

        let actions = fsm.handle(ExecutionInput::Resume {
            model_override: None,
        });

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            ExecutionAction::EmitResumed { id } => {
                assert_eq!(*id, 1);
            }
            _ => panic!("Expected EmitResumed action"),
        }

        match fsm.state() {
            ExecutionState::Running { .. } => {}
            _ => panic!("Expected Running state after resume"),
        }
    }

    #[test]
    fn test_abort_from_running() {
        let mut fsm = create_test_fsm();
        let started_at = Utc::now();
        fsm.handle(ExecutionInput::Start {
            scenario_name: "test".to_string(),
            started_at,
        });

        let actions = fsm.handle(ExecutionInput::Abort);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            ExecutionAction::EmitAborted { id } => {
                assert_eq!(*id, 1);
            }
            _ => panic!("Expected EmitAborted action"),
        }

        match fsm.state() {
            ExecutionState::Aborted { .. } => {}
            _ => panic!("Expected Aborted state"),
        }
    }

    #[test]
    fn test_abort_from_paused() {
        let mut fsm = create_test_fsm();
        let started_at = Utc::now();
        fsm.handle(ExecutionInput::Start {
            scenario_name: "test".to_string(),
            started_at,
        });
        fsm.handle(ExecutionInput::Pause {
            error: "error".to_string(),
            step_name: "step1".to_string(),
        });

        let actions = fsm.handle(ExecutionInput::Abort);

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            ExecutionAction::EmitAborted { id } => {
                assert_eq!(*id, 1);
            }
            _ => panic!("Expected EmitAborted action"),
        }

        match fsm.state() {
            ExecutionState::Aborted { .. } => {}
            _ => panic!("Expected Aborted state"),
        }
    }

    #[test]
    fn test_finish_from_running() {
        let mut fsm = create_test_fsm();
        let started_at = Utc::now();
        fsm.handle(ExecutionInput::Start {
            scenario_name: "test".to_string(),
            started_at,
        });

        let finished_at = Utc::now();
        let actions = fsm.handle(ExecutionInput::Finish { finished_at });

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            ExecutionAction::EmitFinished { id, scenario_name, .. } => {
                assert_eq!(*id, 1);
                assert_eq!(scenario_name, "test");
            }
            _ => panic!("Expected EmitFinished action"),
        }

        match fsm.state() {
            ExecutionState::Finished { .. } => {}
            _ => panic!("Expected Finished state"),
        }
    }

    #[test]
    fn test_invalid_transition_start_from_running() {
        let mut fsm = create_test_fsm();
        let started_at = Utc::now();
        fsm.handle(ExecutionInput::Start {
            scenario_name: "test".to_string(),
            started_at,
        });

        let actions = fsm.handle(ExecutionInput::Start {
            scenario_name: "test2".to_string(),
            started_at: Utc::now(),
        });

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            ExecutionAction::Error { message } => {
                assert!(message.contains("Invalid transition"));
            }
            _ => panic!("Expected Error action"),
        }
    }

    #[test]
    fn test_add_token_usage() {
        let mut fsm = create_test_fsm();
        let started_at = Utc::now();
        fsm.handle(ExecutionInput::Start {
            scenario_name: "test".to_string(),
            started_at,
        });

        let usage = TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        };
        let actions = fsm.handle(ExecutionInput::AddTokenUsage { usage: usage.clone() });

        assert_eq!(actions.len(), 0);

        match fsm.state() {
            ExecutionState::Running { token_usage, .. } => {
                assert_eq!(token_usage.total_tokens, 30);
            }
            _ => panic!("Expected Running state"),
        }
    }
}
