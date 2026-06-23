use std::collections::HashMap;

use rhd_ai::config::ModelConfig;
use rhd_ai::OpenAiClient;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

use super::placeholder::{resolve_placeholders, ExecutionContext, StepResult};
use super::{Action, Scenario};
use crate::log::{LogSink, OutputLine};

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
    scenario_name: &str,
    models: &HashMap<String, ModelConfig>,
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
                let result = execute_run_command(cmd, &context, sink, &step_name, client_cwd).await;
                context.record_step(step_name, result);
            }
            Action::AiChat(chat) => {
                let step_name = chat.name.clone().unwrap_or_else(|| "aiChat".to_string());
                let result =
                    execute_ai_chat(chat, &context, models, default_model, scenario_name, &step_name, sink)
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

async fn execute_run_command(
    cmd: &super::RunCommandAction,
    context: &ExecutionContext,
    sink: &mut LogSink,
    step_name: &str,
    client_cwd: &str,
) -> StepResult {
    let resolved_command = resolve_placeholders(&cmd.command, context);
    let resolved_args: Vec<String> = cmd
        .args
        .iter()
        .map(|arg| resolve_placeholders(arg, context))
        .collect();

    sink.log_step(
        step_name,
        "running command",
        &format!("{} {}", resolved_command, resolved_args.join(" ")),
    );

    let mut command = tokio::process::Command::new(&resolved_command);
    command.args(&resolved_args);

    let resolved_cwd = match &cmd.working_dir {
        Some(dir) => resolve_placeholders(dir, context),
        None => client_cwd.to_string(),
    };
    command.current_dir(&resolved_cwd);

    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    match command.spawn() {
        Ok(mut child) => {
            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();

            let (tx, mut rx) = mpsc::channel::<OutputLine>(100);

            let tx_out = tx.clone();
            let stdout_task = tokio::spawn(async move {
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let _ = tx_out.send(OutputLine::Stdout(line.clone())).await;
                        }
                        Err(_) => break,
                    }
                }
            });

            let tx_err = tx.clone();
            let stderr_task = tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let _ = tx_err.send(OutputLine::Stderr(line.clone())).await;
                        }
                        Err(_) => break,
                    }
                }
            });

            drop(tx);

            let mut output_lines = Vec::new();
            let mut stdout_buf = String::new();
            let mut stderr_buf = String::new();

            while let Some(output_line) = rx.recv().await {
                match &output_line {
                    OutputLine::Stdout(s) => stdout_buf.push_str(s),
                    OutputLine::Stderr(s) => stderr_buf.push_str(s),
                }
                output_lines.push(output_line);
            }

            let _ = tokio::join!(stdout_task, stderr_task);
            let status = child.wait().await;

            let (exit_code, success) = match status {
                Ok(s) => (s.code().unwrap_or(-1), s.success()),
                Err(_) => (-1, false),
            };

            sink.log_step(step_name, "command exit code", &exit_code.to_string());
            sink.log_command_output(step_name, &output_lines);

            StepResult {
                exit_code,
                stdout: stdout_buf,
                stderr: stderr_buf,
                stdout_stderr: output_lines,
                success,
                message: None,
                cwd: Some(resolved_cwd),
            }
        }
        Err(err) => {
            sink.log_step(step_name, "command spawn error", &err.to_string());
            StepResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: err.to_string(),
                stdout_stderr: vec![OutputLine::Stderr(err.to_string())],
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
    sink: &mut LogSink,
) -> Result<StepResult, ExecuteError> {
    let model_name = match &chat.model {
        Some(m) => m.clone(),
        None => match default_model {
            Some(m) => m.to_string(),
            None => {
                sink.log_step(
                    step_name,
                    "AI request failed",
                    "no default model configured",
                );
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
            sink.log_step(
                step_name,
                "AI request failed",
                &format!("unknown model '{model_name}'"),
            );
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

    sink.log_ai_request(step_name, &model_name, &system_prompt, &message);

    let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);

    match client.chat(&model_config.model, &system_prompt, &message).await {
        Ok(response) => {
            sink.log_step(step_name, "AI response", &response);
            Ok(StepResult {
                exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
                stdout_stderr: vec![],
                success: true,
                message: Some(response),
                cwd: None,
            })
        }
        Err(err) => {
            sink.log_step(step_name, "AI request failed", &err.to_string());
            Err(ExecuteError::AiFailed {
                scenario: scenario_name.to_string(),
                step: step_name.to_string(),
                message: err.to_string(),
            })
        },
    }
}
