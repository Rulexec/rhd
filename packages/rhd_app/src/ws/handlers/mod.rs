mod scenario;
mod chat;
mod project;
mod role;

use std::sync::Arc;

use rhd_api::{
    ErrorCode, WsRequest, WsResponse,
};

use crate::daemon::DaemonState;

pub async fn handle_ws_message(text: &str, state: &Arc<DaemonState>) -> WsResponse {
    let request: WsRequest = match serde_json::from_str(text) {
        Ok(r) => r,
        Err(e) => {
            return WsResponse::error(
                "unknown".to_string(),
                ErrorCode::InvalidRequest,
                format!("invalid JSON: {}", e),
            );
        }
    };

    match request {
        WsRequest::RunScenario { id, name, cwd, model_aliases } => {
            scenario::handle_run_scenario(id, name, cwd, model_aliases, state).await
        }
        WsRequest::Subscribe { id } => scenario::handle_subscribe(id, state).await,
        WsRequest::GetFinishedScenarios { id, last_id } => scenario::handle_get_finished(id, last_id, state),
        WsRequest::AbortScenario { id, execution_id } => scenario::handle_abort_scenario(id, execution_id, state),
        WsRequest::RetryScenario { id, execution_id, model } => {
            scenario::handle_retry_scenario(id, execution_id, model, state)
        }
        WsRequest::AbortScenarioWithError { id, execution_id } => {
            scenario::handle_abort_scenario_with_error(id, execution_id, state)
        }
        WsRequest::CreateChat { id, title } => chat::handle_create_chat(id, title, state),
        WsRequest::ListChats { id } => chat::handle_list_chats(id, state),
        WsRequest::GetChat { id, chat_id } => chat::handle_get_chat(id, chat_id, state),
        WsRequest::DeleteChat { id, chat_id } => chat::handle_delete_chat(id, chat_id, state),
        WsRequest::DeleteAllChats { id } => chat::handle_delete_all_chats(id, state).await,
        WsRequest::SendMessage { id, chat_id, content, model } => {
            chat::handle_send_message(id, chat_id, content, model, state).await
        }
        WsRequest::EditMessage { id, message_id, content, model } => {
            chat::handle_edit_message(id, message_id, content, model, state).await
        }
        WsRequest::AbortChat { id, chat_id } => chat::handle_abort_chat(id, chat_id, state).await,
        WsRequest::GetAvailableModels { id } => chat::handle_get_available_models(id, state).await,
        WsRequest::DevNotification { id } => chat::handle_dev_notification(id, state),
        WsRequest::PauseChat { id, chat_id } => chat::handle_pause_chat(id, chat_id, state).await,
        WsRequest::ResumeChat { id, chat_id } => chat::handle_resume_chat(id, chat_id, state).await,
        WsRequest::QueueMessage { id, chat_id, content, model } => {
            chat::handle_queue_message(id, chat_id, content, model, state).await
        }
        WsRequest::ListProjects { id } => project::handle_list_projects(id, state).await,
        WsRequest::GetProjectMcpStatus { id, project_name } => {
            project::handle_get_project_mcp_status(id, project_name, state).await
        }
        WsRequest::AttachProject { id, chat_id, project_name } => {
            project::handle_attach_project(id, chat_id, project_name, state).await
        }
        WsRequest::DetachProject { id, chat_id, project_name } => {
            project::handle_detach_project(id, chat_id, project_name, state).await
        }
        WsRequest::GetChatProjects { id, chat_id } => {
            project::handle_get_chat_projects(id, chat_id, state)
        }
        WsRequest::SetRole { id, chat_id, project_name, role_name } => {
            role::handle_set_role(id, chat_id, project_name, role_name, state).await
        }
        WsRequest::GetAvailableRoles { id, chat_id } => {
            role::handle_get_available_roles(id, chat_id, state)
        }
        WsRequest::ClearActiveRole { id, chat_id } => {
            role::handle_clear_active_role(id, chat_id, state).await
        }
    }
}
