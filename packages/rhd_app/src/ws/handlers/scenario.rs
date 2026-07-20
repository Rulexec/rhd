use std::sync::Arc;

use rhd_api::{ErrorCode, WsResponse};

use crate::daemon::DaemonState;
use crate::log::read_finished_scenarios;

pub async fn handle_run_scenario(
    id: String,
    name: String,
    cwd: String,
    model_aliases: Vec<(String, String)>,
    state: &Arc<DaemonState>,
) -> WsResponse {
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

pub async fn handle_subscribe(id: String, state: &Arc<DaemonState>) -> WsResponse {
    let active = state.execution_tracker.get_active_executions();
    let inner = state.inner.read().await;
    
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

pub fn handle_get_finished(id: String, last_id: Option<u64>, state: &Arc<DaemonState>) -> WsResponse {
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

pub fn handle_abort_scenario(id: String, execution_id: u64, state: &Arc<DaemonState>) -> WsResponse {
    state.execution_tracker.abort(execution_id);
    WsResponse::success(id, serde_json::json!({ "aborted": true }))
}

pub fn handle_retry_scenario(id: String, execution_id: u64, model: Option<String>, state: &Arc<DaemonState>) -> WsResponse {
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

pub fn handle_abort_scenario_with_error(id: String, execution_id: u64, state: &Arc<DaemonState>) -> WsResponse {
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
