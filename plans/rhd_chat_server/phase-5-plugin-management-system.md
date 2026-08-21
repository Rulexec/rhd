# Phase 5: Plugin Management System

## Overview

This phase implements the plugin management system and custom event broadcasting. Plugins are WebSocket connections that register themselves with a unique `plugin_id`. The system tracks plugin active status, broadcasts plugin list changes, and provides a custom event system with acknowledgment tracking.

**Scope:**
- Plugin registration, deactivation on disconnect, and removal
- Plugin list subscription system (subscribePluginsList, unsubscribePluginsList)
- Custom event broadcasting to all connected clients
- Custom event acknowledgment tracking
- Pending acknowledgments query
- 5 plugin-related event types: pluginRegistered, pluginRemoved, pluginUpdated, customEvent, customEventAcknowledged
- 8 plugin-related methods: registerPlugin, getPlugins, subscribePluginsList, unsubscribePluginsList, removePlugin, sendCustomEvent, ackCustomEvent, getPendingAcks

**Out of scope:**
- Chat/message operations (Phase 3)
- Chat/chats list subscriptions (Phase 4)

## Dependencies

- **Phase 1: Database Extensions** — Must be completed for plugins, custom_events, custom_event_acks tables
- **Phase 2: WebSocket Server Core** — Must be completed for connection handling
- **Phase 3: Request Handlers** — Must be completed for handler pattern
- **Phase 4: Subscription System** — Must be completed for subscription manager pattern and event broadcasting
- **rhd_chat_api** — Must be implemented for all plugin method and event types

## Architecture

### Plugin Registry

The `PluginRegistry` is a shared state that tracks:
- Mapping from `plugin_id` to `connection_id` (which connection owns the plugin)
- Mapping from `connection_id` to set of `plugin_id`s (for cleanup on disconnect)
- Set of connection IDs subscribed to the plugins list

When a connection disconnects, all its plugins are marked as inactive in the database.

### Custom Event Flow

1. Client calls `sendCustomEvent` with event name and optional data
2. Server generates a UUID for the event
3. Event is stored in `custom_events` table
4. `customEvent` is broadcast to ALL connected WebSocket clients
5. Plugins can call `ackCustomEvent` to acknowledge
6. Acknowledgment is stored in `custom_event_acks` table
7. `customEventAcknowledged` is sent to the original sender's connection

### Connection-to-Plugin Mapping

Each connection can register multiple plugins. The registry tracks:
- `plugin_to_connection: HashMap<String, String>` — plugin_id → connection_id
- `connection_to_plugins: HashMap<String, HashSet<String>>` — connection_id → set of plugin_ids

On disconnect, all plugins for that connection are marked inactive.

## Files to Create

### 1. `packages/rhd_chat_server/src/plugins.rs`

**Create plugin registry:**

```rust
//! Plugin registry for tracking plugin-to-connection mappings.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::sync::RwLock;

use rhd_db::ChatDb;

use crate::error::ServerError;
use crate::subscriptions::{ConnectionId, EventSender};

/// Plugin registry that tracks plugin-to-connection mappings.
#[derive(Default)]
pub struct PluginRegistry {
    /// Map from plugin_id to connection_id.
    plugin_to_connection: HashMap<String, ConnectionId>,
    /// Map from connection_id to set of plugin_ids.
    connection_to_plugins: HashMap<ConnectionId, HashSet<String>>,
}

impl PluginRegistry {
    /// Create a new plugin registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a plugin for a connection.
    /// Returns true if this is a new plugin, false if it was re-registered.
    pub fn register_plugin(&mut self, plugin_id: &str, connection_id: &str) -> bool {
        let is_new = !self.plugin_to_connection.contains_key(plugin_id);

        // Remove from old connection if re-registering
        if let Some(old_connection_id) = self.plugin_to_connection.get(plugin_id) {
            if old_connection_id != connection_id {
                if let Some(plugins) = self.connection_to_plugins.get_mut(old_connection_id) {
                    plugins.remove(plugin_id);
                }
            }
        }

        self.plugin_to_connection.insert(plugin_id.to_string(), connection_id.to_string());
        self.connection_to_plugins
            .entry(connection_id.to_string())
            .or_insert_with(HashSet::new)
            .insert(plugin_id.to_string());

        is_new
    }

    /// Get the connection ID for a plugin.
    pub fn get_connection_for_plugin(&self, plugin_id: &str) -> Option<&str> {
        self.plugin_to_connection.get(plugin_id).map(|s| s.as_str())
    }

    /// Get all plugin IDs for a connection.
    pub fn get_plugins_for_connection(&self, connection_id: &str) -> Vec<String> {
        self.connection_to_plugins
            .get(connection_id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Remove all plugins for a connection and return their IDs.
    /// Called when a connection disconnects.
    pub fn remove_plugins_for_connection(&mut self, connection_id: &str) -> Vec<String> {
        let plugin_ids = self.connection_to_plugins.remove(connection_id).unwrap_or_default();
        for plugin_id in &plugin_ids {
            self.plugin_to_connection.remove(plugin_id);
        }
        plugin_ids.into_iter().collect()
    }

    /// Remove a specific plugin.
    pub fn remove_plugin(&mut self, plugin_id: &str) -> Option<String> {
        if let Some(connection_id) = self.plugin_to_connection.remove(plugin_id) {
            if let Some(plugins) = self.connection_to_plugins.get_mut(&connection_id) {
                plugins.remove(plugin_id);
            }
            Some(connection_id)
        } else {
            None
        }
    }

    /// Check if a plugin is registered.
    pub fn has_plugin(&self, plugin_id: &str) -> bool {
        self.plugin_to_connection.contains_key(plugin_id)
    }
}

/// Thread-safe wrapper for plugin registry.
pub type SharedPluginRegistry = Arc<RwLock<PluginRegistry>>;

/// Create a new shared plugin registry.
pub fn new_shared_plugin_registry() -> SharedPluginRegistry {
    Arc::new(RwLock::new(PluginRegistry::new()))
}

/// Deactivate all plugins for a connection when it disconnects.
pub async fn deactivate_plugins_on_disconnect(
    db: &ChatDb,
    plugin_registry: &SharedPluginRegistry,
    connection_id: &str,
) -> Result<Vec<String>, ServerError> {
    let mut registry = plugin_registry.write().await;
    let plugin_ids = registry.remove_plugins_for_connection(connection_id);

    for plugin_id in &plugin_ids {
        db.deactivate_plugin(plugin_id)?;
    }

    Ok(plugin_ids)
}
```

### 2. `packages/rhd_chat_server/src/custom_events.rs`

**Create custom event helpers:**

```rust
//! Custom event broadcasting and acknowledgment helpers.

use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

use rhd_chat_api::common::PendingEvent;
use rhd_chat_api::events::{CustomEventAcknowledgedData, CustomEventData};
use rhd_chat_api::protocol::Event;

use rhd_db::ChatDb;

use crate::error::ServerError;

/// Create and store a custom event, returning the event and its ID.
pub fn create_custom_event(
    db: &ChatDb,
    event_name: &str,
    sender_plugin_id: Option<&str>,
    additional: Option<&str>,
) -> Result<(String, Event), ServerError> {
    let event_id = Uuid::new_v4().to_string();
    let created_at = Utc::now();

    // Store in database
    db.create_custom_event(&event_id, event_name, sender_plugin_id, additional)?;

    // Create event for broadcasting
    let data = CustomEventData {
        event_id: event_id.clone(),
        event_name: event_name.to_string(),
        sender_plugin_id: sender_plugin_id.map(|s| s.to_string()),
        additional: additional.map(|s| s.to_string()),
        created_at,
    };

    let event = Event::new("customEvent", serde_json::to_value(data)?);

    Ok((event_id, event))
}

/// Acknowledge a custom event and create the acknowledgment event.
pub fn ack_custom_event(
    db: &ChatDb,
    event_id: &str,
    plugin_id: &str,
) -> Result<Option<Event>, ServerError> {
    // Check if event exists
    let event_info = db.get_custom_event(event_id)?;
    let event_info = match event_info {
        Some(e) => e,
        None => return Ok(None),
    };

    // Store acknowledgment
    db.ack_custom_event(event_id, plugin_id)?;

    // Create acknowledgment event for the sender
    if let Some(sender_plugin_id) = &event_info.sender_plugin_id {
        let data = CustomEventAcknowledgedData {
            event_id: event_id.to_string(),
            acknowledging_plugin_id: plugin_id.to_string(),
        };
        let event = Event::new("customEventAcknowledged", serde_json::to_value(data)?);
        Ok(Some(event))
    } else {
        Ok(None)
    }
}

/// Get pending events for a plugin and convert to API types.
pub fn get_pending_events(
    db: &ChatDb,
    plugin_id: &str,
) -> Result<Vec<PendingEvent>, ServerError> {
    let events = db.get_pending_events_for_plugin(plugin_id)?;
    let pending_events = events
        .into_iter()
        .map(|e| {
            let created_at = e.created_at.parse().unwrap_or_else(|_| Utc::now());
            PendingEvent {
                event_id: e.event_id,
                event_name: e.event_name,
                sender_plugin_id: e.sender_plugin_id,
                additional: e.additional,
                created_at,
            }
        })
        .collect();
    Ok(pending_events)
}
```

### 3. `packages/rhd_chat_server/src/handlers/plugin.rs`

**Create plugin handlers:**

```rust
//! Plugin operation handlers.

use serde_json::Value;

use rhd_chat_api::common::{PendingEvent, PluginSummary};
use rhd_chat_api::events::{PluginRegisteredData, PluginRemovedData, PluginUpdatedData};
use rhd_chat_api::methods::{
    AckCustomEventParams, AckCustomEventResult, GetPendingAcksParams, GetPendingAcksResult,
    GetPluginsParams, GetPluginsResult, RegisterPluginParams, RegisterPluginResult,
    RemovePluginParams, RemovePluginResult, SendCustomEventParams, SendCustomEventResult,
    SubscribePluginsListParams, SubscribePluginsListResult, UnsubscribePluginsListParams,
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
) -> Result<Response, ServerError> {
    let params: RegisterPluginParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
        }
    };

    // Register in plugin registry
    let is_new = {
        let mut registry = plugin_registry.write().await;
        registry.register_plugin(&params.plugin_id, connection_id)
    };

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
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `getPlugins` request.
pub async fn get_plugins(
    _params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Response, ServerError> {
    let db_plugins = db.get_plugins()?;
    let plugins: Vec<PluginSummary> = db_plugins
        .into_iter()
        .map(|p| PluginSummary {
            plugin_id: p.plugin_id,
            is_active: p.is_active,
        })
        .collect();

    let result = GetPluginsResult { plugins };
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `subscribePluginsList` request.
pub async fn subscribe_plugins_list(
    _params: Value,
    _db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    let mut manager = subscription_manager.write().await;
    manager.subscribe_plugins_list(connection_id);

    let result = SubscribePluginsListResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `unsubscribePluginsList` request.
pub async fn unsubscribe_plugins_list(
    _params: Value,
    _db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    let mut manager = subscription_manager.write().await;
    manager.unsubscribe_plugins_list(connection_id);

    let result = UnsubscribePluginsListResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `removePlugin` request.
pub async fn remove_plugin(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    plugin_registry: SharedPluginRegistry,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    let params: RemovePluginParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
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
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `sendCustomEvent` request.
pub async fn send_custom_event(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    plugin_registry: SharedPluginRegistry,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    let params: SendCustomEventParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
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
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `ackCustomEvent` request.
pub async fn ack_custom_event(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    plugin_registry: SharedPluginRegistry,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    let params: AckCustomEventParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
        }
    };

    // Find the plugin_id for this connection
    let plugin_id = {
        let registry = plugin_registry.read().await;
        let plugins = registry.get_plugins_for_connection(connection_id);
        match plugins.into_iter().next() {
            Some(p) => p,
            None => {
                return Ok(ErrorResponse::invalid_request(
                    request_id,
                    "Connection has no registered plugin",
                ));
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
                event_info.sender_plugin_id.and_then(|sender_id| {
                    registry.get_connection_for_plugin(&sender_id).map(|s| s.to_string())
                })
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
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `getPendingAcks` request.
pub async fn get_pending_acks(
    _params: Value,
    db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    plugin_registry: SharedPluginRegistry,
) -> Result<Response, ServerError> {
    // Find the plugin_id for this connection
    let plugin_id = {
        let registry = plugin_registry.read().await;
        let plugins = registry.get_plugins_for_connection(connection_id);
        match plugins.into_iter().next() {
            Some(p) => p,
            None => {
                return Ok(ErrorResponse::invalid_request(
                    request_id,
                    "Connection has no registered plugin",
                ));
            }
        }
    };

    let pending_events = custom_events::get_pending_events(db, &plugin_id)?;

    let result = GetPendingAcksResult { pending_events };
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}
```

### 4. Update `packages/rhd_chat_server/src/subscriptions.rs`

**Extend subscription manager for plugins list and broadcast-to-all:**

Add to `SubscriptionManager`:

```rust
/// Set of connection IDs subscribed to the plugins list.
plugins_list_subscribers: HashSet<ConnectionId>,

/// Subscribe a connection to the plugins list.
pub fn subscribe_plugins_list(&mut self, connection_id: &str) {
    self.plugins_list_subscribers.insert(connection_id.to_string());
}

/// Unsubscribe a connection from the plugins list.
pub fn unsubscribe_plugins_list(&mut self, connection_id: &str) {
    self.plugins_list_subscribers.remove(connection_id);
}

/// Broadcast an event to all subscribers of the plugins list.
pub fn broadcast_to_plugins_list(&self, event: Event) {
    for connection_id in &self.plugins_list_subscribers {
        if let Some(sender) = self.connections.get(connection_id) {
            let _ = sender.send(event.clone());
        }
    }
}

/// Broadcast an event to ALL connected clients.
pub fn broadcast_to_all(&self, event: Event) {
    for sender in self.connections.values() {
        let _ = sender.send(event.clone());
    }
}

/// Send an event to a specific connection.
pub fn send_to_connection(&self, connection_id: &str, event: Event) {
    if let Some(sender) = self.connections.get(connection_id) {
        let _ = sender.send(event);
    }
}
```

Also update `unregister_connection` to clean up `plugins_list_subscribers`:

```rust
pub fn unregister_connection(&mut self, connection_id: &str) {
    // ... existing cleanup ...
    self.plugins_list_subscribers.remove(connection_id);
    // ... rest of cleanup ...
}
```

### 5. Update `packages/rhd_chat_server/src/handlers/mod.rs`

**Add plugin handlers to router:**

```rust
pub mod chat;
pub mod message;
pub mod plugin;
pub mod subscription;

use rhd_chat_api::protocol::{Request, Response};
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;
use crate::plugins::SharedPluginRegistry;
use crate::subscriptions::SharedSubscriptionManager;

/// Route a request to the appropriate handler.
pub async fn handle_request(
    request: Request,
    db: &ChatDb,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
    plugin_registry: SharedPluginRegistry,
) -> Result<Response, ServerError> {
    let request_id = request.id.clone();
    
    match request.method.as_str() {
        // Chat methods
        "createChat" => chat::create_chat(request.params, db, &request_id, &subscription_manager).await,
        "listChats" => chat::list_chats(request.params, db, &request_id).await,
        "getChat" => chat::get_chat(request.params, db, &request_id).await,
        "deleteChat" => chat::delete_chat(request.params, db, &request_id, &subscription_manager).await,
        "updateChat" => chat::update_chat(request.params, db, &request_id, &subscription_manager).await,
        
        // Message methods
        "addMessage" => message::add_message(request.params, db, &request_id, &subscription_manager).await,
        "updateMessage" => message::update_message(request.params, db, &request_id, &subscription_manager).await,
        "deleteMessage" => message::delete_message(request.params, db, &request_id, &subscription_manager).await,
        
        // Subscription methods
        "subscribeChat" => subscription::subscribe_chat(request.params, db, &request_id, connection_id, subscription_manager).await,
        "unsubscribeChat" => subscription::unsubscribe_chat(request.params, db, &request_id, connection_id, subscription_manager).await,
        "subscribeChatsList" => subscription::subscribe_chats_list(request.params, db, &request_id, connection_id, subscription_manager).await,
        "unsubscribeChatsList" => subscription::unsubscribe_chats_list(request.params, db, &request_id, connection_id, subscription_manager).await,
        
        // Plugin methods
        "registerPlugin" => plugin::register_plugin(request.params, db, &request_id, connection_id, plugin_registry.clone(), subscription_manager).await,
        "getPlugins" => plugin::get_plugins(request.params, db, &request_id).await,
        "subscribePluginsList" => plugin::subscribe_plugins_list(request.params, db, &request_id, connection_id, subscription_manager).await,
        "unsubscribePluginsList" => plugin::unsubscribe_plugins_list(request.params, db, &request_id, connection_id, subscription_manager).await,
        "removePlugin" => plugin::remove_plugin(request.params, db, &request_id, plugin_registry, subscription_manager).await,
        "sendCustomEvent" => plugin::send_custom_event(request.params, db, &request_id, connection_id, plugin_registry, subscription_manager).await,
        "ackCustomEvent" => plugin::ack_custom_event(request.params, db, &request_id, connection_id, plugin_registry, subscription_manager).await,
        "getPendingAcks" => plugin::get_pending_acks(request.params, db, &request_id, connection_id, plugin_registry).await,
        
        // Unknown method
        _ => Ok(ErrorResponse::invalid_request(
            request_id,
            format!("Unknown method: {}", request.method),
        )),
    }
}
```

### 6. Update `packages/rhd_chat_server/src/connection.rs`

**Integrate plugin registry and cleanup on disconnect:**

```rust
use crate::plugins::{self, SharedPluginRegistry};

/// Handle a single WebSocket connection.
pub async fn handle_connection(
    mut read: WsRead,
    mut write: WsWrite,
    db: Arc<ChatDb>,
    subscription_manager: SharedSubscriptionManager,
    plugin_registry: SharedPluginRegistry,
) -> Result<(), ServerError> {
    // Register connection
    let (connection_id, mut event_receiver) = {
        let mut manager = subscription_manager.write().await;
        manager.register_connection()
    };

    debug!("Connection registered: {}", connection_id);

    // ... write task spawning ...

    // Process incoming messages
    let result = process_messages(read, write, db.clone(), subscription_manager.clone(), plugin_registry.clone(), &connection_id).await;

    // Deactivate plugins for this connection
    if let Err(e) = plugins::deactivate_plugins_on_disconnect(&db, &plugin_registry, &connection_id).await {
        error!("Failed to deactivate plugins for connection {}: {}", connection_id, e);
    }

    // Broadcast pluginUpdated events for deactivated plugins
    // (This should be done after deactivation to get the updated status)

    // Unregister connection
    {
        let mut manager = subscription_manager.write().await;
        manager.unregister_connection(&connection_id);
    }

    // ... cleanup ...

    result
}
```

Update `process_messages` to pass `plugin_registry`:

```rust
async fn process_messages(
    mut read: WsRead,
    mut write: WsWrite,
    db: Arc<ChatDb>,
    subscription_manager: SharedSubscriptionManager,
    plugin_registry: SharedPluginRegistry,
    connection_id: &str,
) -> Result<(), ServerError> {
    // ... in message handling ...
    let response = match handlers::handle_request(
        request,
        &db,
        connection_id,
        subscription_manager.clone(),
        plugin_registry.clone(),
    ).await {
        // ...
    };
}
```

### 7. Update `packages/rhd_chat_server/src/server.rs`

**Pass plugin registry to connection handler:**

```rust
use crate::plugins::new_shared_plugin_registry;

/// Run the WebSocket server.
pub async fn run(config: Config) -> Result<(), ServerError> {
    // Initialize database
    let db = Arc::new(ChatDb::new(&config.db_path)?);
    info!("Database initialized at {}", config.db_path);

    // Create subscription manager
    let subscription_manager = new_shared_subscription_manager();
    info!("Subscription manager initialized");

    // Create plugin registry
    let plugin_registry = new_shared_plugin_registry();
    info!("Plugin registry initialized");

    // Bind TCP listener
    let listener = TcpListener::bind(&config.socket_addr()).await?;
    info!("WebSocket server listening on ws://{}/", config.socket_addr());

    // Accept connections
    loop {
        let (stream, addr) = listener.accept().await?;
        info!("New connection from: {}", addr);

        let db = Arc::clone(&db);
        let subscription_manager = subscription_manager.clone();
        let plugin_registry = plugin_registry.clone();
        tokio::spawn(async move {
            match accept_async(stream).await {
                Ok(ws_stream) => {
                    let (write, read) = ws_stream.split();
                    if let Err(e) = handle_connection(read, write, db, subscription_manager, plugin_registry).await {
                        error!("Connection error from {}: {}", addr, e);
                    }
                    info!("Connection closed: {}", addr);
                }
                Err(e) => {
                    error!("WebSocket handshake failed for {}: {}", addr, e);
                }
            }
        });
    }
}
```

### 8. Update `packages/rhd_chat_server/src/main.rs`

**Add new modules:**

```rust
mod config;
mod connection;
mod custom_events;
mod error;
mod handlers;
mod plugins;
mod server;
mod subscriptions;
```

## Implementation Notes

1. **Plugin-to-Connection Mapping**: Each connection can register multiple plugins, but typically one. The registry tracks both directions: plugin → connection and connection → plugins.

2. **Disconnect Cleanup**: When a connection disconnects, `deactivate_plugins_on_disconnect()` is called to:
   - Remove all plugins from the registry
   - Mark them as inactive in the database
   - Optionally broadcast `pluginUpdated` events

3. **Custom Event Broadcasting**: `sendCustomEvent` broadcasts to ALL connected clients using `broadcast_to_all()`, not just subscribers. This is by design — custom events are global.

4. **Acknowledgment Flow**: When a plugin acknowledges an event:
   - The acknowledgment is stored in the database
   - A `customEventAcknowledged` event is created
   - The event is sent to the original sender's connection (found via plugin registry)

5. **Sender Plugin Detection**: For `sendCustomEvent` and `ackCustomEvent`, the server looks up which plugin(s) are registered for the calling connection. If none, `ackCustomEvent` and `getPendingAcks` return an error.

6. **Thread Safety**: Both `PluginRegistry` and `SubscriptionManager` use `Arc<RwLock<...>>` for shared access. Write locks are held briefly.

7. **Event Ordering**: Plugin events (`pluginRegistered`, `pluginRemoved`, `pluginUpdated`) are broadcast to plugins list subscribers. Custom events are broadcast to all connections.

## Testing

### Manual Testing

1. **Register plugin:**
   ```json
   {"type": "request", "id": "1", "method": "registerPlugin", "params": {"pluginId": "test-plugin"}}
   ```
   Expected: Success, subscribers receive `pluginRegistered` event

2. **Get plugins:**
   ```json
   {"type": "request", "id": "2", "method": "getPlugins", "params": {}}
   ```
   Expected: List with registered plugin

3. **Subscribe to plugins list:**
   ```json
   {"type": "request", "id": "3", "method": "subscribePluginsList", "params": {}}
   ```
   Expected: Success

4. **Send custom event:**
   ```json
   {"type": "request", "id": "4", "method": "sendCustomEvent", "params": {"eventName": "test-event", "additional": "{\"key\": \"value\"}"}}
   ```
   Expected: All connections receive `customEvent`, result contains `eventId`

5. **Ack custom event (from another plugin connection):**
   ```json
   {"type": "request", "id": "5", "method": "ackCustomEvent", "params": {"eventId": "<event-id>"}}
   ```
   Expected: Success, sender receives `customEventAcknowledged`

6. **Get pending acks:**
   ```json
   {"type": "request", "id": "6", "method": "getPendingAcks", "params": {}}
   ```
   Expected: List of unacknowledged events

7. **Remove plugin:**
   ```json
   {"type": "request", "id": "7", "method": "removePlugin", "params": {"pluginId": "test-plugin"}}
   ```
   Expected: Success, subscribers receive `pluginRemoved` event

### Disconnect Test

1. Register plugin on connection A
2. Close connection A
3. Call `getPlugins` from connection B
4. Verify plugin shows `isActive: false`

## Success Criteria

1. Plugins can be registered, listed, and removed
2. Plugin active status is tracked correctly
3. Plugins are deactivated when their connection disconnects
4. Plugin list changes are broadcast to subscribers
5. Custom events are broadcast to all connected clients
6. Custom events are stored in the database
7. Acknowledgments are tracked per plugin
8. `customEventAcknowledged` is sent to the original sender
9. `getPendingAcks` returns correct unacknowledged events
10. All 20 API methods work correctly

## Next Steps

After this phase is complete, proceed to **Phase 6: Integration and Testing** to wire everything together and add comprehensive tests.
