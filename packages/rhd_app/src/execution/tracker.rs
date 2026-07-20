use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use rhd_api::{EventData, EventType, ExecutionEvent, ScenarioMeta, TokenUsage};
use rhd_db::ScenarioDb;
use tokio::sync::{broadcast, oneshot};

use super::{
    AbortHandle, ActiveExecution, ActiveExecutionInfo, ExecutionHandle, ExecutionState,
    ExecutionTracker, PausedState, PauseNotification, ResumeAction,
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
        let abort_handle = AbortHandle::new();
        let abort_receiver = abort_handle.receiver();

        {
            let mut active = self.active.lock().unwrap();
            active.insert(
                id,
                ActiveExecution {
                    scenario_name: scenario_name.clone(),
                    started_at,
                    abort_handle,
                    pause_state: Mutex::new(None),
                    resume_tx: Mutex::new(None),
                },
            );
        }

        let _ = self.events_tx.send(ExecutionEvent {
            event: EventType::ScenarioStarted,
            data: EventData::ScenarioStarted {
                id,
                name: scenario_name.clone(),
                daemon_time: Utc::now(),
                started_at,
            },
        });

        Arc::new(ExecutionHandle {
            id,
            tracker: self.clone(),
            scenario_name,
            started_at,
            state: Mutex::new(ExecutionState {
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
        let active = self.active.lock().unwrap();
        if let Some(exec) = active.get(&id) {
            exec.abort_handle.abort();
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ExecutionEvent> {
        self.events_tx.subscribe()
    }

    pub fn get_active_executions(&self) -> Vec<ActiveExecutionInfo> {
        let active = self.active.lock().unwrap();
        active
            .iter()
            .map(|(id, e)| ActiveExecutionInfo {
                id: *id,
                scenario_name: e.scenario_name.clone(),
                started_at: e.started_at,
            })
            .collect()
    }

    pub fn pause(&self, id: u64, error: String, step_name: String) -> Option<oneshot::Receiver<ResumeAction>> {
        let active = self.active.lock().unwrap();
        if let Some(exec) = active.get(&id) {
            let (tx, rx) = oneshot::channel();
            *exec.pause_state.lock().unwrap() = Some(PausedState { error: error.clone(), step_name: step_name.clone() });
            *exec.resume_tx.lock().unwrap() = Some(tx);
            
            let _ = self.pause_notify_tx.send(PauseNotification {
                execution_id: id,
                error,
                step_name,
            });
            
            Some(rx)
        } else {
            None
        }
    }
    
    pub fn subscribe_pause(&self) -> broadcast::Receiver<PauseNotification> {
        self.pause_notify_tx.subscribe()
    }

    pub fn resume(&self, id: u64, model_override: Option<String>) -> bool {
        let active = self.active.lock().unwrap();
        if let Some(exec) = active.get(&id) {
            *exec.pause_state.lock().unwrap() = None;
            if let Some(tx) = exec.resume_tx.lock().unwrap().take() {
                let _ = tx.send(ResumeAction::Retry { model_override });
                let _ = self.events_tx.send(ExecutionEvent {
                    event: EventType::ScenarioResumed,
                    data: EventData::ScenarioResumed { execution_id: id },
                });
                return true;
            }
        }
        false
    }

    pub fn abort_paused(&self, id: u64) -> bool {
        let active = self.active.lock().unwrap();
        if let Some(exec) = active.get(&id) {
            *exec.pause_state.lock().unwrap() = None;
            if let Some(tx) = exec.resume_tx.lock().unwrap().take() {
                let _ = tx.send(ResumeAction::Abort);
                return true;
            }
        }
        false
    }

    pub fn get_paused_state(&self, id: u64) -> Option<PausedState> {
        let active = self.active.lock().unwrap();
        if let Some(exec) = active.get(&id) {
            exec.pause_state.lock().unwrap().clone()
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
}
