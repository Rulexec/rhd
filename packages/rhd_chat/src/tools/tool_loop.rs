use std::sync::Arc;

use rhd_ai::client::{OpenAiClient, ToolCall};
use rhd_ai::ToolDefinition;
use rhd_db::ChatDb;
use rhd_mcp_client::client::McpClient;
use rhd_mcp_client::{McpClientTrait, ToolResult};
use tokio::sync::{broadcast, Notify};
use tokio_util::sync::CancellationToken;

use crate::chat_log::ChatLoggers;
use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::stream::TemplateLoaderRef;
use crate::ProjectProvider;

use super::builtin::{
    RHD_SET_ROLE_TOOL_NAME, RHD_SET_TODO_LIST_TOOL_NAME, handle_rhd_set_role,
    handle_rhd_set_todo_list,
};
use super::db_sync_listener::create_db_sync_listener;
use super::fsm_wrapper::FsmToolLoop;
use super::utils::split_tool_name;

#[derive(Debug)]
pub enum ToolLoopResult {
    Completed { message_id: i64 },
    Paused,
}

pub async fn collect_tools_from_projects<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    template_loader: &TemplateLoaderRef,
) -> (Vec<ToolDefinition>, Vec<(String, String, Arc<McpClient>)>) {
    let mut tools = Vec::new();
    let mut mcp_clients = Vec::new();

    tools.extend(super::builtin::collect_builtin_tools(db, project_provider, chat_id, template_loader));

    let attached_projects = match db.get_chat_projects(chat_id) {
        Ok(projects) => projects,
        Err(_) => return (tools, mcp_clients),
    };

    for (project_name, _) in attached_projects {
        let clients = project_provider.get_mcp_clients(&project_name).await;
        for (mcp_id, client) in clients {
            if let Ok(client_tools) = client.list_tools().await {
                for tool in client_tools {
                    let prefixed_name = format!("{}/{}", mcp_id, tool.name);
                    tools.push(ToolDefinition {
                        tool_type: "function".to_string(),
                        function: rhd_ai::FunctionDefinition {
                            name: prefixed_name,
                            description: tool.description,
                            parameters: tool.input_schema,
                        },
                    });
                }
                mcp_clients.push((project_name.clone(), mcp_id, client));
            }
        }
    }

    (tools, mcp_clients)
}

/// Main tool loop function - now implemented using FSM
pub async fn tool_loop<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    model: &str,
    api_model: &str,
    client: &OpenAiClient,
    tools: &[ToolDefinition],
    mcp_clients: &[(String, String, Arc<McpClient>)],
    cancel_token: &CancellationToken,
    event_sender: &broadcast::Sender<ChatEvent>,
    pause_notify: Arc<Notify>,
    iterations: &mut u32,
    current_content: &mut String,
    max_iterations: u32,
    loggers: Option<ChatLoggers>,
    template_loader: &crate::stream::TemplateLoaderRef,
) -> Result<ToolLoopResult, ChatError> {
    // Create DB sync listener
    let db_sync_listener = create_db_sync_listener(
        manager.db().clone(),
        chat_id,
        event_sender.clone(),
    );

    // Create FSM wrapper
    let mut fsm_loop = FsmToolLoop::new(
        manager,
        chat_id,
        model.to_string(),
        api_model.to_string(),
        client,
        mcp_clients.to_vec(),
        cancel_token.clone(),
        event_sender.clone(),
        pause_notify.clone(),
        max_iterations,
        loggers,
        template_loader,
    );

    // Run the FSM loop with DB sync listener
    // Listener is registered after loading initial messages to avoid duplicate insert errors
    let result = fsm_loop.run(Some(db_sync_listener)).await;

    // Update iterations counter from FSM state
    *iterations = fsm_loop.iterations();

    result
}

pub async fn execute_tool_call<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    tool_call: &ToolCall,
    mcp_clients: &[(String, String, Arc<McpClient>)],
    event_sender: &broadcast::Sender<ChatEvent>,
) -> (ToolResult, String) {
    let tool_name = &tool_call.function.name;

    if tool_name == RHD_SET_TODO_LIST_TOOL_NAME {
        let result = handle_rhd_set_todo_list(manager, chat_id, &tool_call.function.arguments, event_sender).await;
        return (result, String::new());
    }

    if tool_name == RHD_SET_ROLE_TOOL_NAME {
        let result = handle_rhd_set_role(manager, chat_id, &tool_call.function.arguments, event_sender).await;
        return (result, String::new());
    }

    let (mcp_id, bare_tool_name) = split_tool_name(tool_name);
    for (_project_name, client_mcp_id, client) in mcp_clients {
        if *client_mcp_id == mcp_id {
            match client.call_tool(&bare_tool_name, &tool_call.function.arguments).await {
                Ok(result) => return (result, client_mcp_id.clone()),
                Err(e) => return (ToolResult {
                    content: format!("Error: {}", e),
                    is_error: Some(true),
                    raw_response: None,
                }, client_mcp_id.clone()),
            }
        }
    }
    (ToolResult {
        content: format!("Error: unknown tool '{}'", tool_call.function.name),
        is_error: Some(true),
        raw_response: None,
    }, String::new())
}
