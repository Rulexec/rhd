use std::collections::HashMap;

use rhd_ai::config::ModelConfig;
use rhd_mcp_client::McpConfig;

use super::ai_chat::execute_ai_chat;
use super::error::{ExecuteError, ExecuteOutput};
use super::placeholder::{resolve_placeholders, ExecutionContext};
use super::run_command::execute_run_command;
use super::{Action, Scenario};
use crate::log::LogSink;
use crate::mcp_cache::McpServerCache;

pub async fn execute_scenario(
    scenario: &Scenario,
    scenario_name: &str,
    models: &HashMap<String, ModelConfig>,
    mcp_configs: &HashMap<String, McpConfig>,
    mcp_cache: &McpServerCache,
    default_model: Option<&str>,
    sink: &mut LogSink,
    client_cwd: &str,
) -> Result<ExecuteOutput, ExecuteError> {
    let mut context = ExecutionContext::default();
    let mut outputs = Vec::new();

    sink.log(scenario_name, "executing scenario", "");

    for action in &scenario.actions {
        match action {
            Action::RunCommand(cmd) => {
                let step_name = cmd.name.clone().unwrap_or_else(|| cmd.command.clone());
                
                if let Some(skip_expr) = &cmd.skip {
                    if evaluate_skip(skip_expr, &context) {
                        sink.log(&step_name, "skipped", skip_expr);
                        continue;
                    }
                }
                
                let result = execute_run_command(cmd, &context, sink, &step_name, client_cwd).await;
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
                
                let result =
                    execute_ai_chat(chat, &mut context, models, mcp_configs, mcp_cache, default_model, scenario_name, &step_name, sink)
                        .await?;
                context.record_step(step_name, result);
            }
            Action::Output(output) => {
                let step_name = output.name.clone().unwrap_or_else(|| "output".to_string());
                let resolved = resolve_placeholders(&output.text, &context);
                sink.log(&step_name, "output step", &resolved);
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
