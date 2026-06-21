use std::collections::HashMap;

use rhd_ai::config::ModelConfig;
use rhd_ai::OpenAiClient;
use thiserror::Error;

use super::placeholder::{resolve_placeholders, ExecutionContext, StepResult};
use super::{Action, Scenario};

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("scenario '{scenario}' step '{step}': unknown model '{model}'")]
    UnknownModel {
        scenario: String,
        step: String,
        model: String,
    },

    #[error("scenario '{scenario}' step '{step}': no default model configured")]
    NoDefaultModel { scenario: String, step: String },

    #[error("scenario '{scenario}' step '{step}': command failed: {message}")]
    CommandFailed {
        scenario: String,
        step: String,
        message: String,
    },

    #[error("scenario '{scenario}' step '{step}': AI request failed: {message}")]
    AiFailed {
        scenario: String,
        step: String,
        message: String,
    },
}

#[derive(Debug)]
pub struct ExecuteOutput {
    pub outputs: Vec<String>,
    #[allow(dead_code)]
    pub context: ExecutionContext,
}

pub async fn execute_scenario(
    scenario: &Scenario,
    models: &HashMap<String, ModelConfig>,
    default_model: Option<&str>,
) -> Result<ExecuteOutput, ExecuteError> {
    let mut context = ExecutionContext::default();
    let mut outputs = Vec::new();
    let scenario_name = &scenario.name;

    for action in &scenario.actions {
        match action {
            Action::RunCommand(cmd) => {
                let step_name = cmd.name.clone().unwrap_or_else(|| cmd.command.clone());
                let result = execute_run_command(cmd, &context).await;
                if !result.success {
                    return Err(ExecuteError::CommandFailed {
                        scenario: scenario_name.clone(),
                        step: step_name,
                        message: if result.stderr.is_empty() {
                            format!("exit code {}", result.exit_code)
                        } else {
                            result.stderr.clone()
                        },
                    });
                }
                context.record_step(step_name, result);
            }
            Action::AiChat(chat) => {
                let step_name = chat.name.clone().unwrap_or_else(|| "aiChat".to_string());
                let result =
                    execute_ai_chat(chat, &context, models, default_model, scenario_name, &step_name)
                        .await?;
                context.record_step(step_name, result);
            }
            Action::Output(output) => {
                let resolved = resolve_placeholders(&output.text, &context);
                outputs.push(resolved);
            }
        }
    }

    Ok(ExecuteOutput { outputs, context })
}

async fn execute_run_command(
    cmd: &super::RunCommandAction,
    context: &ExecutionContext,
) -> StepResult {
    let resolved_command = resolve_placeholders(&cmd.command, context);
    let resolved_args: Vec<String> = cmd
        .args
        .iter()
        .map(|arg| resolve_placeholders(arg, context))
        .collect();

    let mut command = tokio::process::Command::new(&resolved_command);
    command.args(&resolved_args);

    if let Some(working_dir) = &cmd.working_dir {
        let resolved_dir = resolve_placeholders(working_dir, context);
        command.current_dir(resolved_dir);
    }

    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    match command.output().await {
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let success = output.status.success();

            StepResult {
                exit_code,
                stdout,
                stderr,
                success,
                message: None,
            }
        }
        Err(err) => StepResult {
            exit_code: -1,
            stdout: String::new(),
            stderr: err.to_string(),
            success: false,
            message: None,
        },
    }
}

async fn execute_ai_chat(
    chat: &super::AiChatAction,
    context: &ExecutionContext,
    models: &HashMap<String, ModelConfig>,
    default_model: Option<&str>,
    scenario_name: &str,
    step_name: &str,
) -> Result<StepResult, ExecuteError> {
    let model_name = match &chat.model {
        Some(m) => m.clone(),
        None => match default_model {
            Some(m) => m.to_string(),
            None => {
                return Err(ExecuteError::NoDefaultModel {
                    scenario: scenario_name.to_string(),
                    step: step_name.to_string(),
                });
            }
        },
    };

    let model_config = match models.get(&model_name) {
        Some(config) => config.clone(),
        None => {
            return Err(ExecuteError::UnknownModel {
                scenario: scenario_name.to_string(),
                step: step_name.to_string(),
                model: model_name,
            });
        }
    };

    let system_prompt = chat
        .system_prompt
        .as_ref()
        .map(|s| resolve_placeholders(s, context))
        .unwrap_or_default();

    let message = resolve_placeholders(&chat.message, context);

    let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);

    match client.chat(&model_config.model, &system_prompt, &message).await {
        Ok(response) => Ok(StepResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            success: true,
            message: Some(response),
        }),
        Err(err) => Err(ExecuteError::AiFailed {
            scenario: scenario_name.to_string(),
            step: step_name.to_string(),
            message: err.to_string(),
        }),
    }
}
