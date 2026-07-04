use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use rhd_api::{EventData, EventType, ExecutionEvent, LogSection, ScenarioMeta, ScenarioStatus, StepTiming, StepType, TokenUsage};
use rhd_db::ScenarioDb;
use tokio::sync::{broadcast, watch, oneshot};

#[derive(Clone, Debug)]
pub struct PauseNotification {
    pub execution_id: u64,
    pub error: String,
    pub step_name: String,
}

pub struct ExecutionTracker {
    db: Arc<ScenarioDb>,
    active: Mutex<HashMap<u64, ActiveExecution>>,
    events_tx: broadcast::Sender<ExecutionEvent>,
    pause_notify_tx: broadcast::Sender<PauseNotification>,
}

#[derive(Clone, Debug)]
pub struct PausedState {
    pub error: String,
    pub step_name: String,
}

#[derive(Clone, Debug)]
pub enum ResumeAction {
    Retry { model_override: Option<String> },
    Abort,
}

struct ActiveExecution {
    scenario_name: String,
    started_at: DateTime<Utc>,
    abort_handle: AbortHandle,
    pause_state: Mutex<Option<PausedState>>,
    resume_tx: Mutex<Option<oneshot::Sender<ResumeAction>>>,
}

pub struct AbortHandle {
    sender: watch::Sender<bool>,
}

impl AbortHandle {
    pub fn new() -> Self {
        let (sender, _) = watch::channel(false);
        Self { sender }
    }

    pub fn abort(&self) {
        let _ = self.sender.send(true);
    }

    pub fn receiver(&self) -> watch::Receiver<bool> {
        self.sender.subscribe()
    }
}

struct ExecutionState {
    step_timings: Vec<StepTiming>,
    token_usage: TokenUsage,
    current_step_start: Option<DateTime<Utc>>,
    current_step_name: Option<String>,
    current_step_type: Option<StepType>,
    current_step_exit_code: Option<i32>,
    current_step_model: Option<String>,
    current_step_tokens: TokenUsage,
    current_step_sections: Vec<LogSection>,
}

pub struct ExecutionHandle {
    id: u64,
    tracker: Arc<ExecutionTracker>,
    scenario_name: String,
    started_at: DateTime<Utc>,
    state: Mutex<ExecutionState>,
    abort_receiver: watch::Receiver<bool>,
}

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

    fn remove_active(&self, id: u64) {
        let mut active = self.active.lock().unwrap();
        active.remove(&id);
    }
}

pub struct ActiveExecutionInfo {
    pub id: u64,
    pub scenario_name: String,
    pub started_at: DateTime<Utc>,
}

impl ExecutionHandle {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn step_started(&self, step_name: &str, step_type: StepType) {
        let now = Utc::now();
        let mut state = self.state.lock().unwrap();
        Self::finalize_current_step(&mut state, now);

        state.current_step_start = Some(now);
        state.current_step_name = Some(step_name.to_string());
        state.current_step_type = Some(step_type);
        state.current_step_exit_code = None;
        state.current_step_model = None;
        state.current_step_tokens = TokenUsage::default();
        state.current_step_sections = Vec::new();

        let _ = self.tracker.events_tx.send(ExecutionEvent {
            event: EventType::StepStarted,
            data: EventData::StepStarted {
                execution_id: self.id,
                step_name: step_name.to_string(),
                started_at: now,
            },
        });
    }

    pub fn add_token_usage(&self, usage: &TokenUsage) {
        let mut state = self.state.lock().unwrap();
        state.token_usage.add(usage);
        state.current_step_tokens.add(usage);
    }

    pub fn add_section(&self, section: LogSection) {
        let mut state = self.state.lock().unwrap();
        state.current_step_sections.push(section);
    }

    pub fn set_step_exit_code(&self, exit_code: i32) {
        let mut state = self.state.lock().unwrap();
        state.current_step_exit_code = Some(exit_code);
    }

    pub fn set_step_model(&self, model: String) {
        let mut state = self.state.lock().unwrap();
        state.current_step_model = Some(model);
    }

    pub fn abort_signal(&self) -> watch::Receiver<bool> {
        self.abort_receiver.clone()
    }

    pub async fn pause_and_wait(&self, error: String, step_name: String) -> ResumeAction {
        let rx = self.tracker.pause(self.id, error, step_name);
        if let Some(rx) = rx {
            match rx.await {
                Ok(action) => action,
                Err(_) => ResumeAction::Abort,
            }
        } else {
            ResumeAction::Abort
        }
    }

    pub fn finished(self: Arc<Self>, status: ScenarioStatus) -> FinishedExecution {
        let now = Utc::now();
        let mut state = self.state.lock().unwrap();
        Self::finalize_current_step(&mut state, now);
        self.tracker.remove_active(self.id);

        let tokens = if state.token_usage.total_tokens > 0 {
            Some(state.token_usage.clone())
        } else {
            None
        };

        let meta = ScenarioMeta {
            id: self.id,
            scenario: self.scenario_name.clone(),
            status,
            started: self.started_at,
            finished: now,
            duration_ms: (now - self.started_at).num_milliseconds() as u64,
            tokens: tokens.clone(),
            cost: None,
            steps: state.step_timings.clone(),
        };

        let _ = self.tracker.events_tx.send(ExecutionEvent {
            event: EventType::ScenarioFinished,
            data: EventData::ScenarioFinished(meta),
        });

        FinishedExecution {
            id: self.id,
            started_at: self.started_at,
            finished_at: now,
            duration_ms: (now - self.started_at).num_milliseconds() as u64,
            step_timings: state.step_timings.clone(),
            token_usage: tokens,
        }
    }

    fn finalize_current_step(state: &mut ExecutionState, now: DateTime<Utc>) {
        if let (Some(start), Some(name), Some(step_type)) = (
            state.current_step_start.take(),
            state.current_step_name.take(),
            state.current_step_type.take(),
        ) {
            let timing = StepTiming {
                name,
                step_type,
                exit_code: state.current_step_exit_code.take(),
                model: state.current_step_model.take(),
                started_at: start,
                finished_at: now,
                duration_ms: (now - start).num_milliseconds() as u64,
                tokens: if state.current_step_tokens.total_tokens > 0 {
                    Some(state.current_step_tokens.clone())
                } else {
                    None
                },
                cost: None,
                sections: std::mem::take(&mut state.current_step_sections),
            };
            state.step_timings.push(timing);
        }
    }
}

pub struct FinishedExecution {
    pub id: u64,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub step_timings: Vec<StepTiming>,
    pub token_usage: Option<TokenUsage>,
}
