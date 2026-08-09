use std::sync::Arc;

use rhd_ai::client::OpenAiClient;
use rhd_ai::config::ModelConfig;
use tokio::sync::broadcast;

use crate::chat_log;
use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::tools;
use crate::tools::tool_loop::ToolLoopResult;
use crate::ProjectProvider;

use crate::stream::TemplateLoaderRef;

pub(super) async fn send_message_with_tools<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    content: String,
    model: &str,
    model_config: &ModelConfig,
    tool_defs: Vec<rhd_ai::ToolDefinition>,
    mcp_clients: Vec<(
        String,
        String,
        Arc<rhd_mcp_client::client::McpClient>,
    )>,
    event_sender: broadcast::Sender<ChatEvent>,
    loggers: Option<chat_log::ChatLoggers>,
    template_loader: &TemplateLoaderRef,
) -> Result<i64, ChatError> {
    const MAX_ITERATIONS: u32 = 20;

    let (cancel_token, pause_notify) = manager.register_stream(chat_id).await;

    let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);
    let mut iterations = 0u32;
    let mut current_content = content;

    let result = tools::tool_loop(
        manager,
        chat_id,
        model,
        &model_config.model,
        &client,
        &tool_defs,
        &mcp_clients,
        &cancel_token,
        &event_sender,
        pause_notify,
        &mut iterations,
        &mut current_content,
        MAX_ITERATIONS,
        loggers,
        template_loader,
    )
    .await;

    match result {
        Ok(ToolLoopResult::Completed { message_id }) => {
            manager.unregister_stream(chat_id).await;
            Ok(message_id)
        }
        Ok(ToolLoopResult::Paused) => {
            // Stream is paused, don't unregister
            // Tools have already been executed and their results persisted
            // The assistant message with tool calls was already saved in the tool loop
            Ok(0) // Return dummy message_id
        }
        Err(ChatError::Aborted) => {
            // StreamAborted already emitted by tool_loop; keep Aborted state for resume
            Err(ChatError::Aborted)
        }
        Err(e) => {
            let is_aborted = manager.is_aborted(chat_id).await;
            if !is_aborted {
                manager.unregister_stream(chat_id).await;
            }
            Err(e)
        }
    }
}
