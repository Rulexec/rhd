use std::sync::Arc;

use chrono::{DateTime, Utc};
use rhd_api::{EventData, EventType, ExecutionEvent, LogSection, ScenarioMeta, ScenarioStatus, StepTiming, StepType, TokenUsage};
use tokio::sync::watch;

use super::{ExecutionHandle, ExecutionHandleState, FinishedExecution, ResumeAction};

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

        let _ = self.tracker.events_tx().send(ExecutionEvent {
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

        let _ = self.tracker.events_tx().send(ExecutionEvent {
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

    fn finalize_current_step(state: &mut ExecutionHandleState, now: DateTime<Utc>) {
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
