use std::collections::HashMap;

use rhd_ai::config::ModelConfig;
use rhd_ai::OpenAiClient;
use thiserror::Error;

use super::placeholder::{resolve_placeholders, ExecutionContext, StepResult};
use super::{Action, Scenario};

#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("no model configured for aiChat action '{action_name}'")]
    NoModel { action_name: String },

    #[error("unknown model '{model}' referenced in action '{action_name}'")]
    UnknownModel { model: String, action_name: String },

    #[error("no default model configured")]
    NoDefaultModel,
}

#[derive(Debug)]
pub struct ExecuteOutput {
    pub outputs: Vec<String>,
    pub context: ExecutionContext,
}

pub async fn execute_scenario(
    scenario: &Scenario,
    models: &HashMap<String, ModelConfig>,
    default_model: Option<&str>,
) -> Result<ExecuteOutput, ExecuteError> {
    let mut context = ExecutionContext::default();
    let mut outputs = Vec::new();

    for action in &scenario.actions {
        match action {
            Action::RunCommand(cmd) => {
                let result = execute_run_command(cmd, &context).await;
                if let Some(name) = &cmd.name {
                    context.record_step(name.clone(), result);
                }
            }
            Action::AiChat(chat) => {
                let result = execute_ai_chat(chat, &context, models, default_model).await;
                if let Some(name) = &chat.name {
                    context.record_step(name.clone(), result);
                }
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
) -> StepResult {
    let action_name = chat.name.clone().unwrap_or_default();

    let model_name = match &chat.model {
        Some(m) => m.clone(),
        None => match default_model {
            Some(m) => m.to_string(),
            None => {
                return StepResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: String::new(),
                    success: false,
                    message: Some(format!(
                        "no model configured for aiChat action '{action_name}'"
                    )),
                };
            }
        },
    };

    let model_config = match models.get(&model_name) {
        Some(config) => config.clone(),
        None => {
            return StepResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: String::new(),
                success: false,
                message: Some(format!(
                    "unknown model '{model_name}' referenced in action '{action_name}'"
                )),
            };
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
        Ok(response) => StepResult {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
            success: true,
            message: Some(response),
        },
        Err(err) => StepResult {
            exit_code: -1,
            stdout: String::new(),
            stderr: String::new(),
            success: false,
            message: Some(err.to_string()),
        },
    }
}
