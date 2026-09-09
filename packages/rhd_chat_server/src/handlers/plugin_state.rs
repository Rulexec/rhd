//! Plugin state handlers.

use serde_json::Value;

use rhd_chat_api::common::{PluginState, StateFormat, StateVersionRef};
use rhd_chat_api::methods::{
    GetPluginStatesParams, GetPluginStatesResult, RemovePluginStateParams, RemovePluginStateResult,
    SubscribePluginStatesParams, SubscribePluginStatesResult, UnsubscribePluginStatesResult,
    UpdatePluginStateParams, UpdatePluginStateResult,
};
use rhd_chat_api::protocol::Response;
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;
use rhd_db::PluginStateRow;

use crate::error::ServerError;
use crate::events::{plugin_state_changed_event, plugin_state_removed_event};
use crate::subscriptions::SharedSubscriptionManager;

/// Map a DB row to the API type. The CHECK constraint guarantees the format
/// string is one of the two known values; anything else is a bug → error.
fn row_to_api_state(row: PluginStateRow) -> Result<PluginState, ServerError> {
    let format = match row.format.as_str() {
        "markdown" => StateFormat::Markdown,
        "json" => StateFormat::Json,
        other => {
            return Err(ServerError::Internal(format!(
                "invalid stored state format: {other}"
            )))
        }
    };
    Ok(PluginState {
        plugin_id: row.plugin_id,
        key: row.key,
        content: row.content,
        format,
        schema: row.schema,
        version: row.version,
        updated_at: row.updated_at,
    })
}

/// Parse method params, replying with an `invalid_request` error on failure.
fn parse_params<T: serde::de::DeserializeOwned>(
    params: Value,
    request_id: &str,
) -> Result<T, ErrorResponse> {
    serde_json::from_value(params).map_err(|e| {
        ErrorResponse::invalid_request(request_id, format!("Invalid params: {}", e))
    })
}

/// Resolve the owning plugin for write operations; error if none registered.
fn require_plugin_id(
    plugin_id: Option<&str>,
    request_id: &str,
) -> Result<String, ErrorResponse> {
    match plugin_id {
        Some(id) => Ok(id.to_string()),
        None => Err(ErrorResponse::invalid_request(
            request_id,
            "Connection has no registered plugin",
        )),
    }
}

/// Handle `updatePluginState` request.
///
/// Upserts the state owned by the calling plugin, then broadcasts
/// `pluginStateChanged` to plugin-state subscribers. The response carries the
/// stored state including its server-assigned version.
pub async fn update_plugin_state(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    plugin_id: Option<&str>,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let plugin_id = match require_plugin_id(plugin_id, request_id) {
        Ok(id) => id,
        Err(error) => return Ok(serde_json::to_value(error)?),
    };
    let params: UpdatePluginStateParams = match parse_params(params, request_id) {
        Ok(p) => p,
        Err(error) => return Ok(serde_json::to_value(error)?),
    };

    let format_str = match params.format {
        StateFormat::Markdown => "markdown",
        StateFormat::Json => "json",
    };
    let row = db.upsert_plugin_state(
        &plugin_id,
        &params.key,
        &params.content,
        format_str,
        &params.schema,
    )?;
    let state = row_to_api_state(row)?;

    let event = plugin_state_changed_event(state.clone());
    let manager = subscription_manager.read().await;
    manager.broadcast_to_plugin_states(event);

    let result = UpdatePluginStateResult { state };
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `removePluginState` request.
///
/// Tombstones the state (bumping its version) and broadcasts
/// `pluginStateRemoved`. Removing a non-existent or already-removed state is a
/// silent success.
pub async fn remove_plugin_state(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    plugin_id: Option<&str>,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let plugin_id = match require_plugin_id(plugin_id, request_id) {
        Ok(id) => id,
        Err(error) => return Ok(serde_json::to_value(error)?),
    };
    let params: RemovePluginStateParams = match parse_params(params, request_id) {
        Ok(p) => p,
        Err(error) => return Ok(serde_json::to_value(error)?),
    };

    if let Some(version) = db.remove_plugin_state(&plugin_id, &params.key)? {
        let event = plugin_state_removed_event(&plugin_id, &params.key, version);
        let manager = subscription_manager.read().await;
        manager.broadcast_to_plugin_states(event);
    }

    let result = RemovePluginStateResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `getPluginStates` request.
///
/// Returns live (non-removed) states, optionally filtered by plugin and schema.
/// Any connection may read states.
pub async fn get_plugin_states(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Value, ServerError> {
    let params: GetPluginStatesParams = match parse_params(params, request_id) {
        Ok(p) => p,
        Err(error) => return Ok(serde_json::to_value(error)?),
    };

    let rows = db.get_plugin_states(params.plugin_id.as_deref(), params.schema.as_deref())?;
    let states = rows
        .into_iter()
        .map(row_to_api_state)
        .collect::<Result<Vec<_>, _>>()?;

    let result = GetPluginStatesResult { states };
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `subscribePluginStates` request.
///
/// Registers the connection as a plugin-state subscriber under the write lock
/// BEFORE reading the catch-up snapshot, so any update that lands in between is
/// either already in the snapshot or delivered as a live event. Duplicates are
/// made harmless by client-side version gating.
pub async fn subscribe_plugin_states(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: SubscribePluginStatesParams = match parse_params(params, request_id) {
        Ok(p) => p,
        Err(error) => return Ok(serde_json::to_value(error)?),
    };

    {
        let mut manager = subscription_manager.write().await;
        manager.subscribe_plugin_states(connection_id);
    }

    let mut states = Vec::new();
    for StateVersionRef {
        plugin_id,
        key,
        version,
    } in &params.states
    {
        if let Some(row) = db.get_plugin_state(plugin_id, key)? {
            if !row.is_removed && row.version > *version {
                states.push(row_to_api_state(row)?);
            }
        }
    }

    let result = SubscribePluginStatesResult { states };
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `unsubscribePluginStates` request.
pub async fn unsubscribe_plugin_states(
    _params: Value,
    _db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let mut manager = subscription_manager.write().await;
    manager.unsubscribe_plugin_states(connection_id);

    let result = UnsubscribePluginStatesResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}
