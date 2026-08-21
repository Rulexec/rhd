//! Plugin operation handlers.

use serde_json::Value;

use rhd_chat_api::common::PluginSummary;
use rhd_chat_api::events::{PluginRegisteredData, PluginRemovedData};
use rhd_chat_api::methods::{
    AckCustomEventParams, AckCustomEventResult, GetPendingAcksResult, GetPluginsResult,
    RegisterPluginParams, RegisterPluginResult, RemovePluginParams, RemovePluginResult,
    SendCustomEventParams, SendCustomEventResult, SubscribePluginsListResult,
    UnsubscribePluginsListResult,
};
use rhd_chat_api::protocol::{Event, Response};
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::custom_events;
use crate::error::ServerError;
use crate::plugins::SharedPluginRegistry;
use crate::subscriptions::SharedSubscriptionManager;

/// Handle `registerPlugin` request.
pub async fn register_plugin(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    plugin_registry: SharedPluginRegistry,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: RegisterPluginParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Register in plugin registry
    {
        let mut registry = plugin_registry.write().await;
        registry.register_plugin(&params.plugin_id, connection_id);
    }

    // Register/update in database
    db.register_plugin(&params.plugin_id)?;

    // Broadcast pluginRegistered event
    let event_data = PluginRegisteredData {
        plugin_id: params.plugin_id.clone(),
        is_active: true,
    };
    let event = Event::new("pluginRegistered", serde_json::to_value(event_data)?);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_plugins_list(event);

    let result = RegisterPluginResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `getPlugins` request.
pub async fn get_plugins(
    _params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Value, ServerError> {
    let db_plugins = db.get_plugins()?;
    let plugins: Vec<PluginSummary> = db_plugins
        .into_iter()
        .map(|p| PluginSummary {
            plugin_id: p.plugin_id,
            is_active: p.is_active,
        })
        .collect();

    let result = GetPluginsResult { plugins };
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `subscribePluginsList` request.
pub async fn subscribe_plugins_list(
    _params: Value,
    _db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let mut manager = subscription_manager.write().await;
    manager.subscribe_plugins_list(connection_id);

    let result = SubscribePluginsListResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `unsubscribePluginsList` request.
pub async fn unsubscribe_plugins_list(
    _params: Value,
    _db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let mut manager = subscription_manager.write().await;
    manager.unsubscribe_plugins_list(connection_id);

    let result = UnsubscribePluginsListResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `removePlugin` request.
pub async fn remove_plugin(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    plugin_registry: SharedPluginRegistry,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: RemovePluginParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Remove from registry
    {
        let mut registry = plugin_registry.write().await;
        registry.remove_plugin(&params.plugin_id);
    }

    // Remove from database
    db.remove_plugin(&params.plugin_id)?;

    // Broadcast pluginRemoved event
    let event_data = PluginRemovedData {
        plugin_id: params.plugin_id.clone(),
    };
    let event = Event::new("pluginRemoved", serde_json::to_value(event_data)?);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_plugins_list(event);

    let result = RemovePluginResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `sendCustomEvent` request.
pub async fn send_custom_event(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    plugin_registry: SharedPluginRegistry,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: SendCustomEventParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Find sender plugin_id (if this connection has a registered plugin)
    let sender_plugin_id = {
        let registry = plugin_registry.read().await;
        let plugins = registry.get_plugins_for_connection(connection_id);
        plugins.into_iter().next()
    };

    // Create and store event
    let (event_id, event) = custom_events::create_custom_event(
        db,
        &params.event_name,
        sender_plugin_id.as_deref(),
        params.additional.as_deref(),
    )?;

    // Broadcast to ALL connected clients
    let manager = subscription_manager.read().await;
    manager.broadcast_to_all(event);

    let result = SendCustomEventResult { event_id };
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `ackCustomEvent` request.
pub async fn ack_custom_event(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    plugin_registry: SharedPluginRegistry,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: AckCustomEventParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Find the plugin_id for this connection
    let plugin_id = {
        let registry = plugin_registry.read().await;
        let plugins = registry.get_plugins_for_connection(connection_id);
        match plugins.into_iter().next() {
            Some(p) => p,
            None => {
                return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                    request_id,
                    "Connection has no registered plugin",
                ))?);
            }
        }
    };

    // Acknowledge event
    let ack_event = custom_events::ack_custom_event(db, &params.event_id, &plugin_id)?;

    // Send acknowledgment to the original sender
    if let Some(event) = ack_event {
        // Find the sender's connection
        let sender_connection_id = {
            let registry = plugin_registry.read().await;
            // Get the event to find sender
            if let Some(event_info) = db.get_custom_event(&params.event_id)? {
                event_info
                    .sender_plugin_id
                    .and_then(|sender_id| registry.get_connection_for_plugin(&sender_id).map(|s| s.to_string()))
            } else {
                None
            }
        };

        if let Some(sender_conn_id) = sender_connection_id {
            let manager = subscription_manager.read().await;
            manager.send_to_connection(&sender_conn_id, event);
        }
    }

    let result = AckCustomEventResult {};
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}

/// Handle `getPendingAcks` request.
pub async fn get_pending_acks(
    _params: Value,
    db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    plugin_registry: SharedPluginRegistry,
) -> Result<Value, ServerError> {
    // Find the plugin_id for this connection
    let plugin_id = {
        let registry = plugin_registry.read().await;
        let plugins = registry.get_plugins_for_connection(connection_id);
        match plugins.into_iter().next() {
            Some(p) => p,
            None => {
                return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                    request_id,
                    "Connection has no registered plugin",
                ))?);
            }
        }
    };

    let pending_events = custom_events::get_pending_events(db, &plugin_id)?;

    let result = GetPendingAcksResult { pending_events };
    Ok(serde_json::to_value(Response::success(
        request_id,
        serde_json::to_value(result)?,
    ))?)
}
