use std::sync::Arc;

use rhd_api::{ErrorCode, RoleInfo, WsResponse};

use crate::daemon::DaemonState;

pub async fn handle_set_role(
    id: String,
    chat_id: i64,
    project_name: String,
    role_name: String,
    state: &Arc<DaemonState>,
) -> WsResponse {
    match state
        .chat_manager
        .set_active_role(chat_id, &project_name, &role_name, state.chat_event_sender.clone())
        .await
    {
        Ok(()) => WsResponse::success(id, serde_json::json!({ "set": true })),
        Err(rhd_chat::ChatError::ChatNotFound) => WsResponse::error(
            id,
            ErrorCode::ChatNotFound,
            format!("chat not found: {}", chat_id),
        ),
        Err(rhd_chat::ChatError::RoleNotFound(role)) => WsResponse::error(
            id,
            ErrorCode::InvalidRequest,
            format!("role not found: {}", role),
        ),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to set role: {}", err),
        ),
    }
}

pub fn handle_get_available_roles(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.get_available_roles(chat_id) {
        Ok(roles) => {
            let role_infos: Vec<RoleInfo> = roles
                .into_iter()
                .map(|(project_name, role_name, when_to_use)| RoleInfo {
                    project_name,
                    role_name,
                    when_to_use,
                })
                .collect();
            
            let active_role: Option<(String, String)> = state.chat_manager.get_active_role(chat_id).ok().flatten();
            
            let data = serde_json::json!({
                "roles": role_infos,
                "activeRoleProject": active_role.as_ref().map(|(p, _)| p),
                "activeRoleName": active_role.as_ref().map(|(_, r)| r),
            });
            WsResponse::success(id, data)
        }
        Err(rhd_chat::ChatError::ChatNotFound) => WsResponse::error(
            id,
            ErrorCode::ChatNotFound,
            format!("chat not found: {}", chat_id),
        ),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to get available roles: {}", err),
        ),
    }
}

pub async fn handle_clear_active_role(
    id: String,
    chat_id: i64,
    state: &Arc<DaemonState>,
) -> WsResponse {
    match state
        .chat_manager
        .clear_active_role(chat_id, state.chat_event_sender.clone())
    {
        Ok(()) => WsResponse::success(id, serde_json::json!({ "cleared": true })),
        Err(rhd_chat::ChatError::ChatNotFound) => WsResponse::error(
            id,
            ErrorCode::ChatNotFound,
            format!("chat not found: {}", chat_id),
        ),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to clear active role: {}", err),
        ),
    }
}
