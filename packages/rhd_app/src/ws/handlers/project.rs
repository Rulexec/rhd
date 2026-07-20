use std::sync::Arc;

use rhd_api::{ErrorCode, WsResponse};

use crate::daemon::DaemonState;

pub async fn handle_list_projects(id: String, state: &Arc<DaemonState>) -> WsResponse {
    let inner = state.inner.read().await;
    let projects = inner.project_manager.list_projects();
    WsResponse::success(id, serde_json::to_value(&projects).unwrap())
}

pub async fn handle_get_project_mcp_status(
    id: String,
    project_name: String,
    state: &Arc<DaemonState>,
) -> WsResponse {
    let inner = state.inner.read().await;
    if inner.project_manager.get_project(&project_name).is_none() {
        return WsResponse::error(
            id,
            ErrorCode::InvalidRequest,
            format!("project not found: {}", project_name),
        );
    }

    let status_list = inner.project_manager.get_mcp_status(&project_name).await;
    let data: Vec<serde_json::Value> = status_list
        .into_iter()
        .map(|(mcp_id, status)| {
            let (status_str, error) = match status {
                rhd_chat::McpStatus::Connecting => ("connecting", None),
                rhd_chat::McpStatus::Connected => ("connected", None),
                rhd_chat::McpStatus::Failed(ref e) => ("failed", Some(e.clone())),
            };
            let mut obj = serde_json::json!({
                "projectName": project_name,
                "mcpId": mcp_id,
                "status": status_str,
            });
            if let Some(err) = error {
                obj.as_object_mut().unwrap().insert("error".to_string(), serde_json::Value::String(err));
            }
            obj
        })
        .collect();

    WsResponse::success(id, serde_json::to_value(&data).unwrap())
}

pub async fn handle_attach_project(
    id: String,
    chat_id: i64,
    project_name: String,
    state: &Arc<DaemonState>,
) -> WsResponse {
    match state
        .chat_manager
        .attach_project(chat_id, &project_name, state.chat_event_sender.clone())
        .await
    {
        Ok(()) => WsResponse::success(id, serde_json::json!({ "attached": true })),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to attach project: {}", err),
        ),
    }
}

pub async fn handle_detach_project(
    id: String,
    chat_id: i64,
    project_name: String,
    state: &Arc<DaemonState>,
) -> WsResponse {
    match state
        .chat_manager
        .detach_project(chat_id, &project_name, state.chat_event_sender.clone())
        .await
    {
        Ok(()) => WsResponse::success(id, serde_json::json!({ "detached": true })),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to detach project: {}", err),
        ),
    }
}

pub fn handle_get_chat_projects(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.get_chat_projects(chat_id) {
        Ok(projects) => WsResponse::success(id, serde_json::to_value(&projects).unwrap()),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to get chat projects: {}", err),
        ),
    }
}
