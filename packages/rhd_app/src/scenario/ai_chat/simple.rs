use std::collections::HashMap;
use std::sync::Arc;

use rhd_ai::config::ModelConfig;
use rhd_ai::{ChatMessage, OpenAiClient};
use rhd_api::LogSectionKind;

use crate::scenario::error::ExecuteError;
use crate::scenario::executor::ExecutionConfig;
use crate::scenario::placeholder::{ExecutionContext, StepResult};
use super::utils::apply_model_aliases;
use crate::execution::{ExecutionHandle, ResumeAction};
use crate::log::{LogSink, SectionTracker};

pub async fn execute_simple(
    _context: &mut ExecutionContext,
    models: &HashMap<String, ModelConfig>,
    _client: OpenAiClient,
    mut current_model_config: ModelConfig,
    system_prompt: &str,
    message: &str,
    scenario_name: &str,
    step_name: &str,
    sink: &mut LogSink,
    handle: Option<Arc<ExecutionHandle>>,
    model_aliases: &[(String, String)],
    exec_config: &ExecutionConfig,
) -> Result<StepResult, ExecuteError> {
    loop {
        let request_tracker = SectionTracker::start(sink, LogSectionKind::AiRequest);
        sink.log_ai_request(step_name, &current_model_config.model_id, &[], system_prompt, message);
        if let Some(h) = &handle {
            h.add_section(request_tracker.end(sink));
        }
        
        let client = OpenAiClient::new(&current_model_config.base_url, &current_model_config.api_key);
        let messages = vec![ChatMessage::system(system_prompt), ChatMessage::user(message)];
        let chat_future = client.chat_with_tools(&current_model_config.model, messages, &[], None);
        
        let result = if let Some(h) = &handle {
            let mut abort_signal = h.abort_signal();
            tokio::select! {
                res = chat_future => res,
                _ = abort_signal.changed() => {
                    if *abort_signal.borrow() {
                        return Err(ExecuteError::Aborted);
                    }
                    unreachable!()
                }
            }
        } else {
            chat_future.await
        };

        match result {
            Ok(result) => {
                if let Some(usage) = &result.usage {
                    if let Some(h) = &handle {
                        h.add_token_usage(usage);
                    }
                }
                let content = result.content.unwrap_or_default();
                let response_tracker = SectionTracker::start(sink, LogSectionKind::AiResponse);
                sink.log_step(step_name, "AI response", &content);
                if let Some(h) = &handle {
                    h.add_section(response_tracker.end(sink));
                }
                return Ok(StepResult {
                    exit_code: 0,
                    stdout: String::new(),
                    stderr: String::new(),
                    stdout_stderr: vec![],
                    success: true,
                    message: Some(content),
                    cwd: None,
                });
            }
            Err(err) => {
                sink.log_step(step_name, "AI request failed", &err.to_string());
                let execute_error = ExecuteError::AiFailed {
                    scenario: scenario_name.to_string(),
                    step: step_name.to_string(),
                    message: err.to_string(),
                };
                
                let should_pause = exec_config.frontend_alive || exec_config.never_fail;
                if should_pause {
                    if let Some(h) = &handle {
                        let resume_action = h.pause_and_wait(
                            execute_error.to_string(),
                            step_name.to_string(),
                        ).await;
                        
                        match resume_action {
                            ResumeAction::Retry { model_override } => {
                                if let Some(new_model_name) = model_override {
                                    let new_model_name = apply_model_aliases(&new_model_name, model_aliases);
                                    if let Some(new_config) = models.get(&new_model_name) {
                                        current_model_config = new_config.clone();
                                        if let Some(h) = &handle {
                                            h.set_step_model(current_model_config.model_id.clone());
                                        }
                                        continue;
                                    }
                                }
                                continue;
                            }
                            ResumeAction::Abort => {
                                return Err(execute_error);
                            }
                        }
                    }
                }
                return Err(execute_error);
            }
        }
    }
}
