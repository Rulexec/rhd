use std::sync::Arc;

use rhd_api::{
    ErrorCode, RoleInfo, WsRequest, WsResponse,
};

use crate::daemon::DaemonState;
use crate::log::read_finished_scenarios;

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
            handle_run_scenario(id, name, cwd, model_aliases, state).await
        }
        WsRequest::Subscribe { id } => handle_subscribe(id, state).await,
        WsRequest::GetFinishedScenarios { id, last_id } => handle_get_finished(id, last_id, state),
        WsRequest::AbortScenario { id, execution_id } => handle_abort_scenario(id, execution_id, state),
        WsRequest::CreateChat { id, title } => handle_create_chat(id, title, state),
        WsRequest::ListChats { id } => handle_list_chats(id, state),
        WsRequest::GetChat { id, chat_id } => handle_get_chat(id, chat_id, state),
        WsRequest::DeleteChat { id, chat_id } => handle_delete_chat(id, chat_id, state),
        WsRequest::DeleteAllChats { id } => handle_delete_all_chats(id, state).await,
        WsRequest::SendMessage { id, chat_id, content, model } => {
            handle_send_message(id, chat_id, content, model, state).await
        }
        WsRequest::EditMessage { id, message_id, content, model } => {
            handle_edit_message(id, message_id, content, model, state).await
        }
        WsRequest::AbortChat { id, chat_id } => handle_abort_chat(id, chat_id, state).await,
        WsRequest::GetAvailableModels { id } => handle_get_available_models(id, state).await,
        WsRequest::RetryScenario { id, execution_id, model } => {
            handle_retry_scenario(id, execution_id, model, state)
        }
        WsRequest::AbortScenarioWithError { id, execution_id } => {
            handle_abort_scenario_with_error(id, execution_id, state)
        }
        WsRequest::DevNotification { id } => handle_dev_notification(id, state),
        WsRequest::ListProjects { id } => handle_list_projects(id, state).await,
        WsRequest::GetProjectMcpStatus { id, project_name } => {
            handle_get_project_mcp_status(id, project_name, state).await
        }
        WsRequest::AttachProject { id, chat_id, project_name } => {
            handle_attach_project(id, chat_id, project_name, state).await
        }
        WsRequest::DetachProject { id, chat_id, project_name } => {
            handle_detach_project(id, chat_id, project_name, state).await
        }
        WsRequest::GetChatProjects { id, chat_id } => {
            handle_get_chat_projects(id, chat_id, state)
        }
        WsRequest::PauseChat { id, chat_id } => handle_pause_chat(id, chat_id, state).await,
        WsRequest::ResumeChat { id, chat_id } => handle_resume_chat(id, chat_id, state).await,
        WsRequest::SetRole { id, chat_id, project_name, role_name } => {
            handle_set_role(id, chat_id, project_name, role_name, state).await
        }
        WsRequest::GetAvailableRoles { id, chat_id } => {
            handle_get_available_roles(id, chat_id, state)
        }
        WsRequest::ClearActiveRole { id, chat_id } => {
            handle_clear_active_role(id, chat_id, state).await
        }
    }
}

async fn handle_get_available_models(id: String, state: &Arc<DaemonState>) -> WsResponse {
    let inner = state.inner.read().await;
    let mut model_names: Vec<String> = inner
        .models
        .iter()
        .filter(|(_, config)| !config.is_alias)
        .map(|(name, _)| name.clone())
        .collect();
    model_names.sort();
    WsResponse::success(id, serde_json::to_value(&model_names).unwrap())
}

async fn handle_run_scenario(
    id: String,
    name: String,
    cwd: String,
    model_aliases: Vec<(String, String)>,
    state: &Arc<DaemonState>,
) -> WsResponse {
    // Acquire reload_lock read — blocks silently if reload holds write lock
    let _reload_guard = state.reload_lock.read().await;
    let inner = state.inner.read().await;
    let scenario = match inner.scenarios.get(&name) {
        Some(s) => s.clone(),
        None => {
            return WsResponse::error(
                id,
                ErrorCode::UnknownScenario,
                format!("unknown scenario: {}", name),
            );
        }
    };
    let models = inner.models.clone();
    let mcp_configs = inner.mcp_configs.clone();
    let default_model = inner.default_model.clone();
    drop(inner);

    let log_file = state.logs.as_ref().and_then(|logs_dir| {
        crate::log::create_log_dir(logs_dir, &name)
            .and_then(|dir| crate::log::open_log_file(&dir))
            .ok()
    });
    let mut sink = crate::log::LogSink::new(log_file);

    let handle = state.execution_tracker.start(name.clone());

    let exec_config = crate::scenario::ExecutionConfig {
        frontend_alive: state.is_frontend_alive(),
        never_fail: state.never_fail,
    };
    let result = crate::scenario::execute_scenario(
        &scenario,
        &name,
        &models,
        &mcp_configs,
        &*state.mcp_cache,
        default_model.as_deref(),
        &mut sink,
        &cwd,
        Some(handle.clone()),
        &model_aliases,
        &exec_config,
    ).await;

    let status = match &result {
        Ok(_) => rhd_api::ScenarioStatus::Success,
        Err(crate::scenario::ExecuteError::Aborted) => {
            sink.log_aborted();
            rhd_api::ScenarioStatus::Aborted
        }
        Err(_) => rhd_api::ScenarioStatus::Error,
    };

    let finished = handle.finished(status);

    match result {
        Ok(output) => {
            let data = serde_json::json!({
                "output": output.outputs.join("\n"),
                "executionId": finished.id,
            });
            WsResponse::success(id, data)
        }
        Err(crate::scenario::ExecuteError::Aborted) => WsResponse::error(
            id,
            ErrorCode::ScenarioAborted,
            "scenario aborted".to_string(),
        ),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::ScenarioExecutionFailed,
            err.to_string(),
        ),
    }
}

async fn handle_subscribe(id: String, state: &Arc<DaemonState>) -> WsResponse {
    let active = state.execution_tracker.get_active_executions();
    let inner = state.inner.read().await;
    
    // Get paused executions
    let paused: Vec<_> = active.iter().filter_map(|e| {
        state.execution_tracker.get_paused_state(e.id).map(|paused| {
            let model_names: Vec<String> = inner.models.iter()
                .filter(|(_, config)| !config.is_alias)
                .map(|(name, _)| name.clone())
                .collect();
            
            serde_json::json!({
                "executionId": e.id,
                "scenarioName": e.scenario_name,
                "error": paused.error,
                "stepName": paused.step_name,
                "availableModels": model_names,
            })
        })
    }).collect();
    
    let data = serde_json::json!({
        "activeExecutions": active.iter().map(|e| serde_json::json!({
            "id": e.id,
            "scenarioName": e.scenario_name,
            "startedAt": e.started_at,
        })).collect::<Vec<_>>(),
        "pausedExecutions": paused,
    });
    WsResponse::success(id, data)
}

fn handle_get_finished(id: String, last_id: Option<u64>, state: &Arc<DaemonState>) -> WsResponse {
    let scenarios = match state.logs.as_ref() {
        Some(logs_dir) => match read_finished_scenarios(logs_dir) {
            Ok(s) => s,
            Err(_) => Vec::new(),
        },
        None => Vec::new(),
    };

    let filtered = match last_id {
        Some(last) => scenarios.into_iter().filter(|s| s.id > last).collect(),
        None => scenarios,
    };

    let data = serde_json::to_value(filtered).unwrap_or(serde_json::json!([]));
    WsResponse::success(id, data)
}

fn handle_abort_scenario(id: String, execution_id: u64, state: &Arc<DaemonState>) -> WsResponse {
    state.execution_tracker.abort(execution_id);
    WsResponse::success(id, serde_json::json!({ "aborted": true }))
}

fn handle_create_chat(id: String, title: String, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.create_chat(&title) {
        Ok(chat_id) => WsResponse::success(id, serde_json::json!({ "chatId": chat_id })),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to create chat: {}", err),
        ),
    }
}

fn handle_list_chats(id: String, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.list_chats() {
        Ok(chats) => {
            let data = serde_json::to_value(&chats).unwrap_or(serde_json::json!([]));
            WsResponse::success(id, data)
        }
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to list chats: {}", err),
        ),
    }
}

fn handle_get_chat(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.get_chat(chat_id) {
        Ok(Some((chat_info, messages))) => {
            let data = serde_json::json!({
                "chat": chat_info,
                "messages": messages,
            });
            WsResponse::success(id, data)
        }
        Ok(None) => WsResponse::error(
            id,
            ErrorCode::ChatNotFound,
            format!("chat not found: {}", chat_id),
        ),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to get chat: {}", err),
        ),
    }
}

fn handle_delete_chat(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.delete_chat(chat_id) {
        Ok(()) => WsResponse::success(id, serde_json::json!({ "deleted": true })),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to delete chat: {}", err),
        ),
    }
}

async fn handle_delete_all_chats(id: String, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.delete_all_chats().await {
        Ok(()) => WsResponse::success(id, serde_json::json!({ "deleted": true })),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to delete all chats: {}", err),
        ),
    }
}

async fn handle_send_message(
    id: String,
    chat_id: i64,
    content: String,
    model: String,
    state: &Arc<DaemonState>,
) -> WsResponse {
    // Spawn the send_message operation in a separate task to avoid blocking the WebSocket select loop
    let chat_manager = Arc::clone(&state.chat_manager);
    let inner = state.inner.read().await;
    let models = inner.models.clone();
    drop(inner);
    let event_sender = state.chat_event_sender.clone();
    let template_loader = Arc::clone(&state.template_loader);
    
    let state_clone = Arc::clone(state);
    tokio::spawn(async move {
        let template_loader_ref = rhd_chat::stream::TemplateLoaderRef::new(move |name| {
            template_loader.get_template(name).map(|s| s.to_string())
        });
        let _ = chat_manager
            .send_message(chat_id, content, &model, &models, event_sender, &state_clone.reload_lock, &template_loader_ref)
            .await;
    });
    
    // Return immediately with a success response
    WsResponse::success(id, serde_json::json!({ "status": "sending" }))
}

async fn handle_edit_message(
    id: String,
    message_id: i64,
    content: String,
    model: String,
    state: &Arc<DaemonState>,
) -> WsResponse {
    let inner = state.inner.read().await;
    let models = inner.models.clone();
    drop(inner);
    let template_loader = Arc::clone(&state.template_loader);
    let template_loader_ref = rhd_chat::stream::TemplateLoaderRef::new(move |name| {
        template_loader.get_template(name).map(|s| s.to_string())
    });
    match state
        .chat_manager
        .edit_and_resend(
            message_id,
            content,
            &model,
            &models,
            state.chat_event_sender.clone(),
            &state.reload_lock,
            &template_loader_ref,
        )
        .await
    {
        Ok(new_message_id) => {
            WsResponse::success(id, serde_json::json!({ "messageId": new_message_id }))
        }
        Err(rhd_chat::ChatError::MessageNotFound) => WsResponse::error(
            id,
            ErrorCode::MessageNotFound,
            format!("message not found: {}", message_id),
        ),
        Err(rhd_chat::ChatError::ModelNotFound(model_name)) => WsResponse::error(
            id,
            ErrorCode::InvalidRequest,
            format!("model not found: {}", model_name),
        ),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::ChatStreamFailed,
            format!("failed to edit message: {}", err),
        ),
    }
}

async fn handle_abort_chat(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    let aborted = state.chat_manager.abort_chat(chat_id).await;
    WsResponse::success(id, serde_json::json!({ "aborted": aborted }))
}

fn handle_retry_scenario(id: String, execution_id: u64, model: Option<String>, state: &Arc<DaemonState>) -> WsResponse {
    let resumed = state.execution_tracker.resume(execution_id, model);
    if resumed {
        WsResponse::success(id, serde_json::json!({ "resumed": true }))
    } else {
        WsResponse::error(
            id,
            ErrorCode::InvalidRequest,
            format!("execution {} is not paused", execution_id),
        )
    }
}

fn handle_abort_scenario_with_error(id: String, execution_id: u64, state: &Arc<DaemonState>) -> WsResponse {
    let aborted = state.execution_tracker.abort_paused(execution_id);
    if aborted {
        WsResponse::success(id, serde_json::json!({ "aborted": true }))
    } else {
        WsResponse::error(
            id,
            ErrorCode::InvalidRequest,
            format!("execution {} is not paused", execution_id),
        )
    }
}

fn handle_dev_notification(id: String, state: &Arc<DaemonState>) -> WsResponse {
    let _ = state.chat_event_sender.send(rhd_chat::ChatEvent::DevNotification {
        title: "RHD Test".to_string(),
        message: "This is a test notification from rhd dev frontend-notification".to_string(),
    });
    WsResponse::success(id, serde_json::json!({ "sent": true }))
}

async fn handle_list_projects(id: String, state: &Arc<DaemonState>) -> WsResponse {
    let inner = state.inner.read().await;
    let projects = inner.project_manager.list_projects();
    WsResponse::success(id, serde_json::to_value(&projects).unwrap())
}

async fn handle_get_project_mcp_status(
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

async fn handle_attach_project(
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

async fn handle_detach_project(
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

fn handle_get_chat_projects(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    match state.chat_manager.get_chat_projects(chat_id) {
        Ok(projects) => WsResponse::success(id, serde_json::to_value(&projects).unwrap()),
        Err(err) => WsResponse::error(
            id,
            ErrorCode::InternalError,
            format!("failed to get chat projects: {}", err),
        ),
    }
}

async fn handle_pause_chat(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    let paused = state.chat_manager.pause_chat(chat_id).await;
    WsResponse::success(id, serde_json::json!({ "paused": paused }))
}

async fn handle_resume_chat(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
    let resumed = state.chat_manager.resume_chat(chat_id).await;
    WsResponse::success(id, serde_json::json!({ "resumed": resumed }))
}

async fn handle_set_role(
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

fn handle_get_available_roles(id: String, chat_id: i64, state: &Arc<DaemonState>) -> WsResponse {
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

async fn handle_clear_active_role(
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
