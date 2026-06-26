use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use rhd_api::{EventData, EventType, ExecutionEvent, LogSection, StepTiming, TokenUsage};
use tokio::sync::broadcast;

pub struct ExecutionTracker {
    next_id: AtomicU64,
    active: Mutex<HashMap<u64, ActiveExecution>>,
    events_tx: broadcast::Sender<ExecutionEvent>,
}

struct ActiveExecution {
    scenario_name: String,
    started_at: DateTime<Utc>,
}

struct ExecutionState {
    step_timings: Vec<StepTiming>,
    token_usage: TokenUsage,
    current_step_start: Option<DateTime<Utc>>,
    current_step_name: Option<String>,
    current_step_tokens: TokenUsage,
    current_step_sections: Vec<LogSection>,
}

pub struct ExecutionHandle {
    id: u64,
    tracker: Arc<ExecutionTracker>,
    scenario_name: String,
    started_at: DateTime<Utc>,
    state: Mutex<ExecutionState>,
}

impl ExecutionTracker {
    pub fn new() -> Self {
        let (events_tx, _) = broadcast::channel(64);
        Self {
            next_id: AtomicU64::new(1),
            active: Mutex::new(HashMap::new()),
            events_tx,
        }
    }

    pub fn start(self: &Arc<Self>, scenario_name: String) -> Arc<ExecutionHandle> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let started_at = Utc::now();

        {
            let mut active = self.active.lock().unwrap();
            active.insert(
                id,
                ActiveExecution {
                    scenario_name: scenario_name.clone(),
                    started_at,
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
                current_step_tokens: TokenUsage::default(),
                current_step_sections: Vec::new(),
            }),
        })
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

    pub fn step_started(&self, step_name: &str) {
        let now = Utc::now();
        let mut state = self.state.lock().unwrap();
        Self::finalize_current_step(&mut state, now);

        state.current_step_start = Some(now);
        state.current_step_name = Some(step_name.to_string());
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

    pub fn finished(self: Arc<Self>) -> FinishedExecution {
        let now = Utc::now();
        let mut state = self.state.lock().unwrap();
        Self::finalize_current_step(&mut state, now);
        self.tracker.remove_active(self.id);

        let tokens = if state.token_usage.total_tokens > 0 {
            Some(state.token_usage.clone())
        } else {
            None
        };

        let _ = self.tracker.events_tx.send(ExecutionEvent {
            event: EventType::ScenarioFinished,
            data: EventData::ScenarioFinished {
                id: self.id,
                name: self.scenario_name.clone(),
                daemon_time: now,
                started_at: self.started_at,
                finished_at: now,
                duration_ms: (now - self.started_at).num_milliseconds() as u64,
                steps: state.step_timings.clone(),
                tokens: tokens.clone(),
                cost: None,
            },
        });

        FinishedExecution {
            id: self.id,
            scenario_name: self.scenario_name.clone(),
            started_at: self.started_at,
            finished_at: now,
            duration_ms: (now - self.started_at).num_milliseconds() as u64,
            step_timings: state.step_timings.clone(),
            token_usage: tokens,
        }
    }

    fn finalize_current_step(state: &mut ExecutionState, now: DateTime<Utc>) {
        if let (Some(start), Some(name)) = (state.current_step_start.take(), state.current_step_name.take()) {
            let timing = StepTiming {
                name,
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
    pub scenario_name: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub step_timings: Vec<StepTiming>,
    pub token_usage: Option<TokenUsage>,
}
