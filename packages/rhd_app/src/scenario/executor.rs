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
    verbose: bool,
    client_cwd: &str,
) -> Result<ExecuteOutput, ExecuteError> {
    let mut context = ExecutionContext::default();
    let mut outputs = Vec::new();
    let scenario_name = &scenario.name;

    if verbose {
        eprintln!("[verbose] executing scenario '{}'", scenario_name);
    }

    for action in &scenario.actions {
        match action {
            Action::RunCommand(cmd) => {
                let step_name = cmd.name.clone().unwrap_or_else(|| cmd.command.clone());
                let result = execute_run_command(cmd, &context, verbose, client_cwd).await;
                context.record_step(step_name, result);
            }
            Action::AiChat(chat) => {
                let step_name = chat.name.clone().unwrap_or_else(|| "aiChat".to_string());
                let result =
                    execute_ai_chat(chat, &context, models, default_model, scenario_name, &step_name, verbose)
                        .await?;
                context.record_step(step_name, result);
            }
            Action::Output(output) => {
                let resolved = resolve_placeholders(&output.text, &context);
                if verbose {
                    eprintln!("[verbose] output step: {}", resolved);
                }
                outputs.push(resolved);
            }
        }
    }

    Ok(ExecuteOutput { outputs, context })
}

async fn execute_run_command(
    cmd: &super::RunCommandAction,
    context: &ExecutionContext,
    verbose: bool,
    client_cwd: &str,
) -> StepResult {
    let resolved_command = resolve_placeholders(&cmd.command, context);
    let resolved_args: Vec<String> = cmd
        .args
        .iter()
        .map(|arg| resolve_placeholders(arg, context))
        .collect();

    if verbose {
        eprintln!("[verbose] running command: {} {}", resolved_command, resolved_args.join(" "));
    }

    let mut command = tokio::process::Command::new(&resolved_command);
    command.args(&resolved_args);

    let resolved_cwd = match &cmd.working_dir {
        Some(dir) => resolve_placeholders(dir, context),
        None => client_cwd.to_string(),
    };
    command.current_dir(&resolved_cwd);

    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    match command.output().await {
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let success = output.status.success();

            if verbose {
                eprintln!("[verbose] command exit code: {exit_code}");
                if !stdout.is_empty() {
                    eprintln!("[verbose] command stdout:\n{stdout}");
                }
                if !stderr.is_empty() {
                    eprintln!("[verbose] command stderr:\n{stderr}");
                }
                if !success {
                    eprintln!("[verbose] command failed with exit code {exit_code}");
                }
            }

            StepResult {
                exit_code,
                stdout,
                stderr,
                success,
                message: None,
                cwd: Some(resolved_cwd),
            }
        }
        Err(err) => {
            if verbose {
                eprintln!("[verbose] command spawn error: {err}");
            }
            StepResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: err.to_string(),
                success: false,
                message: None,
                cwd: Some(resolved_cwd),
            }
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
    verbose: bool,
) -> Result<StepResult, ExecuteError> {
    let model_name = match &chat.model {
        Some(m) => m.clone(),
        None => match default_model {
            Some(m) => m.to_string(),
            None => {
                if verbose {
                    eprintln!("[verbose] no default model configured for step '{step_name}'");
                }
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
            if verbose {
                eprintln!("[verbose] unknown model '{model_name}' for step '{step_name}'");
            }
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

    if verbose {
        eprintln!("[verbose] AI request to model '{model_name}':");
        if !system_prompt.is_empty() {
            eprintln!("[verbose]   system: {system_prompt}");
        }
        eprintln!("[verbose]   message: {message}");
    }

    let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);

    match client.chat(&model_config.model, &system_prompt, &message).await {
        Ok(response) => {
            if verbose {
                eprintln!("[verbose] AI response: {response}");
            }
            Ok(StepResult {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                success: true,
                message: Some(response),
                cwd: None,
            })
        }
        Err(err) => {
            if verbose {
                eprintln!("[verbose] AI request failed: {err}");
            }
            Err(ExecuteError::AiFailed {
                scenario: scenario_name.to_string(),
                step: step_name.to_string(),
                message: err.to_string(),
            })
        },
    }
}
