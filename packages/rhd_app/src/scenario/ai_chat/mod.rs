mod mcp;
mod simple;
mod utils;

use std::collections::HashMap;
use std::sync::Arc;

use rhd_ai::config::ModelConfig;
use rhd_ai::OpenAiClient;
use rhd_mcp_client::McpConfig;

use super::error::ExecuteError;
use super::executor::ExecutionConfig;
use super::placeholder::{resolve_placeholders, ExecutionContext, StepResult};
use super::AiChatAction;
use crate::execution::ExecutionHandle;
use crate::log::LogSink;
use crate::mcp_cache::McpServerCache;

pub use utils::apply_model_aliases;

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
    exec_config: &ExecutionConfig,
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
        h.set_step_model(model_config.model_id.clone());
    }

    let has_mcp = chat.mcp.as_ref().map(|m| !m.is_empty()).unwrap_or(false);

    if !has_mcp {
        simple::execute_simple(
            context,
            models,
            client,
            model_config,
            &system_prompt,
            &message,
            scenario_name,
            step_name,
            sink,
            handle,
            model_aliases,
            exec_config,
        )
        .await
    } else {
        mcp::execute_with_tools(
            chat,
            context,
            models,
            client,
            model_config.model_id,
            model_config.model,
            &system_prompt,
            &message,
            mcp_configs,
            mcp_cache,
            scenario_name,
            step_name,
            sink,
            handle,
            model_aliases,
            exec_config,
        )
        .await
    }
}
