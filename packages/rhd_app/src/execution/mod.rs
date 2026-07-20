mod handle;
mod tracker;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use rhd_api::{LogSection, StepTiming, StepType, TokenUsage};
use rhd_db::ScenarioDb;
use tokio::sync::{broadcast, oneshot, watch};

#[derive(Clone, Debug)]
pub struct PauseNotification {
    pub execution_id: u64,
    pub error: String,
    pub step_name: String,
}

pub struct ExecutionTracker {
    db: Arc<ScenarioDb>,
    active: Mutex<HashMap<u64, ActiveExecution>>,
    events_tx: broadcast::Sender<rhd_api::ExecutionEvent>,
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

pub struct ActiveExecutionInfo {
    pub id: u64,
    pub scenario_name: String,
    pub started_at: DateTime<Utc>,
}

pub struct FinishedExecution {
    pub id: u64,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub step_timings: Vec<StepTiming>,
    pub token_usage: Option<TokenUsage>,
}
