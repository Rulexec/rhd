use std::collections::HashMap;
use std::sync::Arc;

use rhd_ai::config::ModelConfig;
use rhd_ai::OpenAiClient;
use rhd_api::LogSectionKind;
use rhd_mcp_client::McpConfig;

use super::error::ExecuteError;
use super::placeholder::{resolve_placeholders, ExecutionContext, StepResult};
use super::{AiChatAction, MaxIterations};
use crate::execution::ExecutionHandle;
use crate::log::{LogSink, SectionTracker};
use crate::mcp_cache::McpServerCache;

pub async fn execute_ai_chat(
    chat: &AiChatAction,
    context: &mut ExecutionContext,
    models: &HashMap<String, ModelConfig>,
    mcp_configs: &HashMap<String, McpConfig>,
    mcp_cache: &McpServerCache,
    default_model: Option<&str>,
    scenario_name: &str,
    step_name: &str,
    sink: &mut LogSink,
    handle: Option<Arc<ExecutionHandle>>,
    model_aliases: &[(String, String)],
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

    let model_name = apply_model_aliases(&model_name, model_aliases);

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

    let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);

    if let Some(h) = &handle {
        h.set_step_model(model_name.clone());
    }

    let has_mcp = chat.mcp.as_ref().map(|m| !m.is_empty()).unwrap_or(false);

    if !has_mcp {
        let request_tracker = SectionTracker::start(sink, LogSectionKind::AiRequest);
        sink.log_ai_request(step_name, &model_name, &[], &system_prompt, &message);
        if let Some(h) = &handle {
            h.add_section(request_tracker.end(sink));
        }
        
        let chat_future = client.chat_with_tools(&model_config.model, &system_prompt, &message, &[], &[]);
        
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
                Ok(StepResult {
                    exit_code: 0,
                    stdout: String::new(),
                    stderr: String::new(),
                    stdout_stderr: vec![],
                    success: true,
                    message: Some(content),
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
        execute_ai_chat_with_tools(
            chat,
            context,
            &client,
            &model_name,
            &model_config.model,
            &system_prompt,
            &message,
            mcp_configs,
            mcp_cache,
            scenario_name,
            step_name,
            sink,
            handle,
        )
        .await
    }
}

async fn execute_ai_chat_with_tools(
    chat: &AiChatAction,
    context: &mut ExecutionContext,
    client: &OpenAiClient,
    model_name: &str,
    model: &str,
    system_prompt: &str,
    message: &str,
    mcp_configs: &HashMap<String, McpConfig>,
    mcp_cache: &McpServerCache,
    scenario_name: &str,
    step_name: &str,
    sink: &mut LogSink,
    handle: Option<Arc<ExecutionHandle>>,
) -> Result<StepResult, ExecuteError> {
    use rhd_ai::{FunctionDefinition, ToolDefinition};
    use rhd_mcp_client::McpClientTrait;

    let max_iterations = match &chat.max_tool_iterations {
        Some(MaxIterations::Finite(n)) => *n,
        Some(MaxIterations::Infinite) => u32::MAX,
        None => 20,
    };

    let mut tools = Vec::new();
    let mut mcp_clients = Vec::new();

    if let Some(mcp_refs) = &chat.mcp {
        for mcp_ref in mcp_refs {
            if mcp_ref.name == "flags" {
                tools.push(ToolDefinition {
                    tool_type: "function".to_string(),
                    function: FunctionDefinition {
                        name: "rhd_set_flag".to_string(),
                        description: "Set a named flag with a boolean value".to_string(),
                        parameters: serde_json::json!({
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "value": { "type": "boolean", "default": true }
                            },
                            "required": ["name"]
                        }),
                    },
                });
            } else if let Some(config) = mcp_configs.get(&mcp_ref.name) {
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
                                        tool_type: "function".to_string(),
                                        function: FunctionDefinition {
                                            name: tool.name,
                                            description: tool.description,
                                            parameters: tool.input_schema,
                                        },
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

    let tool_names: Vec<String> = tools.iter().map(|t| t.function.name.clone()).collect();
    let request_tracker = SectionTracker::start(sink, LogSectionKind::AiRequest);
    sink.log_ai_request(step_name, model_name, &tool_names, system_prompt, message);
    if let Some(h) = &handle {
        h.add_section(request_tracker.end(sink));
    }

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

        let chat_future = client.chat_with_tools(model, system_prompt, &current_message, &tools, &tool_results);
        
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

        let result = result.map_err(|e| ExecuteError::AiFailed {
            scenario: scenario_name.to_string(),
            step: step_name.to_string(),
            message: e.to_string(),
        })?;

        if let Some(usage) = &result.usage {
            if let Some(h) = &handle {
                h.add_token_usage(usage);
            }
        }

        if result.tool_calls.is_empty() {
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

        iterations += 1;
        tool_results.clear();

        for tool_call in &result.tool_calls {
            let tool_call_tracker = SectionTracker::start(sink, LogSectionKind::ToolCall);
            sink.log_step(
                step_name,
                "tool call",
                &format!("{}({})", tool_call.name, tool_call.arguments),
            );
            if let Some(h) = &handle {
                h.add_section(tool_call_tracker.end(sink));
            }

            let tool_result = if tool_call.name == "rhd_set_flag" {
                let args: serde_json::Value = serde_json::from_str(&tool_call.arguments)
                    .unwrap_or(serde_json::json!({}));
                let flag_name = args.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let flag_value = args.get("value").and_then(|v| v.as_bool()).unwrap_or(true);
                let full_flag_name = format!("{}.flag_{}", step_name, flag_name);
                context.set_flag(full_flag_name.clone(), flag_value);
                format!("Flag '{}' set to {}", full_flag_name, flag_value)
            } else {
                let mut found = false;
                let mut result_str = String::new();
                for (_mcp_name, mcp_client) in &mcp_clients {
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

            let tool_result_tracker = SectionTracker::start(sink, LogSectionKind::ToolResult);
            sink.log_step(step_name, "tool result", &tool_result);
            if let Some(h) = &handle {
                h.add_section(tool_result_tracker.end(sink));
            }
            tool_results.push((tool_call.id.clone(), tool_result));
        }

        if let Some(content) = &result.content {
            current_message = content.clone();
        }
    }
}

fn apply_model_aliases(model_name: &str, model_aliases: &[(String, String)]) -> String {
    model_aliases
        .iter()
        .find(|(alias, _)| alias == model_name)
        .map(|(_, target)| target.clone())
        .unwrap_or_else(|| model_name.to_string())
}
