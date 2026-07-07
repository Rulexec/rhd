use std::collections::HashMap;
use std::sync::Arc;

use rhd_ai::config::ModelConfig;
use rhd_api::{LogSectionKind, StepType};
use rhd_mcp_client::McpConfig;

use super::ai_chat::execute_ai_chat;
use super::error::{ExecuteError, ExecuteOutput};
use super::placeholder::{resolve_placeholders, ExecutionContext};
use super::run_command::execute_run_command;
use super::{Action, Scenario};
use crate::execution::ExecutionHandle;
use crate::log::{LogSink, SectionTracker};
use crate::mcp_cache::McpServerCache;

#[derive(Clone)]
pub struct ExecutionConfig {
    pub frontend_alive: bool,
    pub never_fail: bool,
}

pub async fn execute_scenario(
    scenario: &Scenario,
    scenario_name: &str,
    models: &HashMap<String, ModelConfig>,
    mcp_configs: &HashMap<String, McpConfig>,
    mcp_cache: &McpServerCache,
    default_model: Option<&str>,
    sink: &mut LogSink,
    client_cwd: &str,
    handle: Option<Arc<ExecutionHandle>>,
    model_aliases: &[(String, String)],
    exec_config: &ExecutionConfig,
) -> Result<ExecuteOutput, ExecuteError> {
    let mut context = ExecutionContext::default();
    let mut outputs = Vec::new();

    sink.log(scenario_name, "executing scenario", "");

    for action in &scenario.actions {
        if let Some(h) = &handle {
            if *h.abort_signal().borrow() {
                return Err(ExecuteError::Aborted);
            }
        }

        match action {
            Action::RunCommand(cmd) => {
                let step_name = cmd.name.clone().unwrap_or_else(|| cmd.command.clone());
                
                if let Some(skip_expr) = &cmd.skip {
                    if evaluate_skip(skip_expr, &context) {
                        sink.log(&step_name, "skipped", skip_expr);
                        continue;
                    }
                }
                
                if let Some(h) = &handle {
                    h.step_started(&step_name, StepType::RunCommand);
                }
                
                let result = execute_run_command(cmd, &context, sink, &step_name, client_cwd, handle.clone()).await;
                context.record_step(step_name, result);
            }
            Action::AiChat(chat) => {
                let step_name = chat.name.clone().unwrap_or_else(|| "aiChat".to_string());
                
                if let Some(skip_expr) = &chat.skip {
                    if evaluate_skip(skip_expr, &context) {
                        sink.log(&step_name, "skipped", skip_expr);
                        continue;
                    }
                }
                
                if let Some(h) = &handle {
                    h.step_started(&step_name, StepType::AiChat);
                }
                
                let result =
                    execute_ai_chat(chat, &mut context, models, mcp_configs, mcp_cache, default_model, scenario_name, &step_name, sink, handle.clone(), model_aliases, exec_config)
                        .await?;
                context.record_step(step_name, result);
            }
            Action::Output(output) => {
                let step_name = output.name.clone().unwrap_or_else(|| "output".to_string());
                
                if let Some(h) = &handle {
                    h.step_started(&step_name, StepType::Output);
                }
                
                let resolved = resolve_placeholders(&output.text, &context);
                let output_tracker = SectionTracker::start(sink, LogSectionKind::OutputStep);
                sink.log(&step_name, "output step", &resolved);
                if let Some(h) = &handle {
                    h.add_section(output_tracker.end(sink));
                }
                outputs.push(resolved);
            }
        }
    }

    Ok(ExecuteOutput { outputs, context })
}

fn evaluate_skip(skip_expr: &str, context: &ExecutionContext) -> bool {
    if let Some(value) = context.get_flag(skip_expr) {
        value
    } else {
        false
    }
}
