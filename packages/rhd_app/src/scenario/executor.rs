use std::collections::HashMap;

use rhd_ai::config::ModelConfig;
use rhd_ai::OpenAiClient;
use rhd_mcp_client::McpConfig;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

use super::placeholder::{resolve_placeholders, ExecutionContext, StepResult};
use super::{Action, Scenario};
use crate::log::{LogSink, OutputLine};
use crate::mcp_cache::McpServerCache;

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

impl From<rhd_ai::AiError> for ExecuteError {
    fn from(err: rhd_ai::AiError) -> Self {
        ExecuteError::AiFailed {
            scenario: String::new(),
            step: String::new(),
            message: err.to_string(),
        }
    }
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
                
                // Check skip condition
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
                
                // Check skip condition
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
    // skip_expr format: "<aiStepName>.flag_<flagName>"
    // e.g., "ai1.flag_skip_build"
    // Skip when flag is TRUE
    if let Some(value) = context.get_flag(skip_expr) {
        value
    } else {
        false
    }
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

            sink.log_step_dashed(step_name, "command exit code", &exit_code.to_string());
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
    context: &mut ExecutionContext,
    models: &HashMap<String, ModelConfig>,
    mcp_configs: &HashMap<String, McpConfig>,
    mcp_cache: &McpServerCache,
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

    // Check if MCP is configured
    let has_mcp = chat.mcp.as_ref().map(|m| !m.is_empty()).unwrap_or(false);

    if !has_mcp {
        // Simple mode: no tools
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
    } else {
        // Tool loop mode
        execute_ai_chat_with_tools(
            chat,
            context,
            &client,
            &model_config.model,
            &system_prompt,
            &message,
            mcp_configs,
            mcp_cache,
            scenario_name,
            step_name,
            sink,
        )
        .await
    }
}

async fn execute_ai_chat_with_tools(
    chat: &super::AiChatAction,
    context: &mut ExecutionContext,
    client: &OpenAiClient,
    model: &str,
    system_prompt: &str,
    message: &str,
    mcp_configs: &HashMap<String, McpConfig>,
    mcp_cache: &McpServerCache,
    scenario_name: &str,
    step_name: &str,
    sink: &mut LogSink,
) -> Result<StepResult, ExecuteError> {
    use rhd_ai::ToolDefinition;
    use rhd_mcp_client::McpClientTrait;

    let max_iterations = match &chat.max_tool_iterations {
        Some(super::MaxIterations::Finite(n)) => *n,
        Some(super::MaxIterations::Infinite) => u32::MAX,
        None => 20, // default
    };

    // Collect tools from all MCP sources
    let mut tools = Vec::new();
    let mut mcp_clients = Vec::new();

    if let Some(mcp_refs) = &chat.mcp {
        for mcp_ref in mcp_refs {
            if mcp_ref.name == "flags" {
                // Built-in flags tool
                tools.push(ToolDefinition {
                    name: "rhd_set_flag".to_string(),
                    description: "Set a named flag with a boolean value".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "value": { "type": "boolean", "default": true }
                        },
                        "required": ["name"]
                    }),
                });
            } else if let Some(config) = mcp_configs.get(&mcp_ref.name) {
                // External MCP server
                let mut resolved_config = config.clone();
                if let Some(args) = &mcp_ref.args {
                    resolved_config.args = args.clone();
                }
                if let Some(env) = &mcp_ref.env {
                    for (k, v) in env {
                        resolved_config.env.insert(k.clone(), v.clone());
                    }
                }

                match mcp_cache.get_or_spawn(&resolved_config).await {
                    Ok(mcp_client) => {
                        match mcp_client.list_tools().await {
                            Ok(mcp_tools) => {
                                for tool in mcp_tools {
                                    tools.push(ToolDefinition {
                                        name: tool.name,
                                        description: tool.description,
                                        input_schema: tool.input_schema,
                                    });
                                }
                                mcp_clients.push((mcp_ref.name.clone(), mcp_client));
                            }
                            Err(e) => {
                                sink.log_step(step_name, "MCP list_tools failed", &e.to_string());
                            }
                        }
                    }
                    Err(e) => {
                        sink.log_step(step_name, "MCP spawn failed", &e.to_string());
                    }
                }
            }
        }
    }

    // Tool call loop
    let mut current_message = message.to_string();
    let mut tool_results: Vec<(String, String)> = Vec::new();
    let mut iterations = 0u32;

    loop {
        if iterations >= max_iterations {
            return Err(ExecuteError::AiFailed {
                scenario: scenario_name.to_string(),
                step: step_name.to_string(),
                message: format!("max tool iterations ({}) exceeded", max_iterations),
            });
        }

        let result = client
            .chat_with_tools(model, system_prompt, &current_message, &tools, &tool_results)
            .await
            .map_err(|e| ExecuteError::AiFailed {
                scenario: scenario_name.to_string(),
                step: step_name.to_string(),
                message: e.to_string(),
            })?;

        if result.tool_calls.is_empty() {
            // No more tool calls, we're done
            let content = result.content.unwrap_or_default();
            sink.log_step(step_name, "AI response", &content);
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

        // Execute tool calls
        iterations += 1;
        tool_results.clear();

        for tool_call in &result.tool_calls {
            sink.log_step(
                step_name,
                "tool call",
                &format!("{}({})", tool_call.name, tool_call.arguments),
            );

            let tool_result = if tool_call.name == "rhd_set_flag" {
                // Built-in flag tool
                let args: serde_json::Value = serde_json::from_str(&tool_call.arguments)
                    .unwrap_or(serde_json::json!({}));
                let flag_name = args.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let flag_value = args.get("value").and_then(|v| v.as_bool()).unwrap_or(true);
                let full_flag_name = format!("{}.flag_{}", step_name, flag_name);
                context.set_flag(full_flag_name.clone(), flag_value);
                format!("Flag '{}' set to {}", full_flag_name, flag_value)
            } else {
                // External MCP tool
                let mut found = false;
                let mut result_str = String::new();
                for (mcp_name, mcp_client) in &mcp_clients {
                    if mcp_client.has_tool(&tool_call.name).await {
                        match mcp_client.call_tool(&tool_call.name, &tool_call.arguments).await {
                            Ok(r) => {
                                result_str = r.content;
                                found = true;
                                break;
                            }
                            Err(e) => {
                                result_str = format!("Error: {}", e);
                                found = true;
                                break;
                            }
                        }
                    }
                }
                if !found {
                    result_str = format!("Error: unknown tool '{}'", tool_call.name);
                }
                result_str
            };

            sink.log_step(step_name, "tool result", &tool_result);
            tool_results.push((tool_call.id.clone(), tool_result));
        }

        // Update message for next iteration (use assistant's content if any)
        if let Some(content) = &result.content {
            current_message = content.clone();
        }
    }
}
