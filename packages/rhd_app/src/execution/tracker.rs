use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use rhd_api::{EventData, EventType, ExecutionEvent, ScenarioMeta, TokenUsage};
use rhd_db::ScenarioDb;
use tokio::sync::{broadcast, oneshot};

use super::{
    ActiveExecutionInfo, ExecutionAction, ExecutionFsm, ExecutionHandle, ExecutionInput,
    ExecutionState as FsmExecutionState, ExecutionTracker, PausedState, PauseNotification,
    ResumeAction,
};

impl ExecutionTracker {
    pub fn new(db: Arc<ScenarioDb>) -> Self {
        let (events_tx, _) = broadcast::channel(64);
        let (pause_notify_tx, _) = broadcast::channel(64);
        Self {
            db,
            active: Mutex::new(HashMap::new()),
            events_tx,
            pause_notify_tx,
        }
    }

    pub fn start(self: &Arc<Self>, scenario_name: String) -> Arc<ExecutionHandle> {
        let id = self.db.next_id().expect("failed to get next scenario ID from database");
        let started_at = Utc::now();

        let mut fsm = ExecutionFsm::new(id);
        let abort_receiver = fsm.abort_receiver();

        // Process Start input through FSM
        let actions = fsm.handle(ExecutionInput::Start {
            scenario_name: scenario_name.clone(),
            started_at,
        });

        // Handle actions from FSM
        self.handle_actions(id, actions);

        {
            let mut active = self.active.lock().unwrap();
            active.insert(id, fsm);
        }

        Arc::new(ExecutionHandle {
            id,
            tracker: self.clone(),
            scenario_name,
            started_at,
            state: Mutex::new(super::ExecutionHandleState {
                step_timings: Vec::new(),
                token_usage: TokenUsage::default(),
                current_step_start: None,
                current_step_name: None,
                current_step_type: None,
                current_step_exit_code: None,
                current_step_model: None,
                current_step_tokens: TokenUsage::default(),
                current_step_sections: Vec::new(),
            }),
            abort_receiver,
        })
    }

    pub fn abort(&self, id: u64) {
        let mut active = self.active.lock().unwrap();
        if let Some(fsm) = active.get_mut(&id) {
            let actions = fsm.handle(ExecutionInput::Abort);
            self.handle_actions(id, actions);
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ExecutionEvent> {
        self.events_tx.subscribe()
    }

    pub fn get_active_executions(&self) -> Vec<ActiveExecutionInfo> {
        let active = self.active.lock().unwrap();
        active
            .iter()
            .filter_map(|(id, fsm)| {
                if let FsmExecutionState::Running { scenario_name, started_at, .. } = fsm.state() {
                    Some(ActiveExecutionInfo {
                        id: *id,
                        scenario_name: scenario_name.clone(),
                        started_at: *started_at,
                    })
                } else if let FsmExecutionState::Paused { scenario_name, started_at, .. } = fsm.state() {
                    Some(ActiveExecutionInfo {
                        id: *id,
                        scenario_name: scenario_name.clone(),
                        started_at: *started_at,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn pause(&self, id: u64, error: String, step_name: String) -> Option<oneshot::Receiver<ResumeAction>> {
        let mut active = self.active.lock().unwrap();
        if let Some(fsm) = active.get_mut(&id) {
            let actions = fsm.handle(ExecutionInput::Pause {
                error: error.clone(),
                step_name: step_name.clone(),
            });

            // Create the receiver before handling actions
            let (tx, rx) = oneshot::channel();

            // Handle actions from FSM
            self.handle_actions(id, actions);

            Some(rx)
        } else {
            None
        }
    }

    pub fn subscribe_pause(&self) -> broadcast::Receiver<PauseNotification> {
        self.pause_notify_tx.subscribe()
    }

    pub fn resume(&self, id: u64, model_override: Option<String>) -> bool {
        let mut active = self.active.lock().unwrap();
        if let Some(fsm) = active.get_mut(&id) {
            // Take the resume_tx before handling Resume
            if let Some(tx) = fsm.take_resume_tx() {
                let actions = fsm.handle(ExecutionInput::Resume { model_override: model_override.clone() });
                self.handle_actions(id, actions);

                let _ = tx.send(ResumeAction::Retry { model_override });
                return true;
            }
        }
        false
    }

    pub fn abort_paused(&self, id: u64) -> bool {
        let mut active = self.active.lock().unwrap();
        if let Some(fsm) = active.get_mut(&id) {
            if let Some(tx) = fsm.take_resume_tx() {
                let actions = fsm.handle(ExecutionInput::Abort);
                self.handle_actions(id, actions);

                let _ = tx.send(ResumeAction::Abort);
                return true;
            }
        }
        false
    }

    pub fn get_paused_state(&self, id: u64) -> Option<PausedState> {
        let active = self.active.lock().unwrap();
        if let Some(fsm) = active.get(&id) {
            if let FsmExecutionState::Paused { pause_reason, step_name, .. } = fsm.state() {
                Some(PausedState {
                    error: pause_reason.clone(),
                    step_name: step_name.clone(),
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn emit_test_scenario_started(&self, id: u64, name: String) {
        let started_at = Utc::now();
        let _ = self.events_tx.send(ExecutionEvent {
            event: EventType::ScenarioStarted,
            data: EventData::ScenarioStarted {
                id,
                name,
                daemon_time: started_at,
                started_at,
            },
        });
    }

    pub fn emit_test_scenario_finished(&self, id: u64, name: String) {
        let now = Utc::now();
        let meta = ScenarioMeta {
            id,
            scenario: name,
            status: rhd_api::ScenarioStatus::Success,
            started: now,
            finished: now,
            duration_ms: 0,
            tokens: None,
            cost: None,
            steps: Vec::new(),
        };
        let _ = self.events_tx.send(ExecutionEvent {
            event: EventType::ScenarioFinished,
            data: EventData::ScenarioFinished(meta),
        });
    }

    pub(super) fn remove_active(&self, id: u64) {
        let mut active = self.active.lock().unwrap();
        active.remove(&id);
    }

    pub(super) fn events_tx(&self) -> &broadcast::Sender<ExecutionEvent> {
        &self.events_tx
    }

    /// Handle actions emitted by the FSM
    fn handle_actions(&self, id: u64, actions: Vec<ExecutionAction>) {
        for action in actions {
            match action {
                ExecutionAction::EmitStarted { id: exec_id, scenario_name, started_at } => {
                    let _ = self.events_tx.send(ExecutionEvent {
                        event: EventType::ScenarioStarted,
                        data: EventData::ScenarioStarted {
                            id: exec_id,
                            name: scenario_name,
                            daemon_time: Utc::now(),
                            started_at,
                        },
                    });
                }
                ExecutionAction::EmitPaused { id: exec_id, error, step_name } => {
                    let _ = self.pause_notify_tx.send(PauseNotification {
                        execution_id: exec_id,
                        error,
                        step_name,
                    });
                }
                ExecutionAction::EmitResumed { id: exec_id } => {
                    let _ = self.events_tx.send(ExecutionEvent {
                        event: EventType::ScenarioResumed,
                        data: EventData::ScenarioResumed { execution_id: exec_id },
                    });
                }
                ExecutionAction::EmitAborted { id: exec_id } => {
                    // Abort event could be emitted here if needed
                    let _ = exec_id;
                }
                ExecutionAction::EmitFinished {
                    id: exec_id,
                    scenario_name,
                    started_at,
                    finished_at,
                    step_timings,
                    token_usage,
                } => {
                    let meta = ScenarioMeta {
                        id: exec_id,
                        scenario: scenario_name,
                        status: rhd_api::ScenarioStatus::Success,
                        started: started_at,
                        finished: finished_at,
                        duration_ms: (finished_at - started_at).num_milliseconds() as u64,
                        tokens: token_usage,
                        cost: None,
                        steps: step_timings,
                    };
                    let _ = self.events_tx.send(ExecutionEvent {
                        event: EventType::ScenarioFinished,
                        data: EventData::ScenarioFinished(meta),
                    });
                }
                ExecutionAction::EmitStepStarted { id: exec_id, step_name, started_at } => {
                    let _ = self.events_tx.send(ExecutionEvent {
                        event: EventType::StepStarted,
                        data: EventData::StepStarted {
                            execution_id: exec_id,
                            step_name,
                            started_at,
                        },
                    });
                }
                ExecutionAction::Error { message } => {
                    tracing::error!(execution_id = id, message = %message, "FSM error");
                }
            }
        }
    }
}
