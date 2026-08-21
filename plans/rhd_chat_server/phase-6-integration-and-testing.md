# Phase 6: Integration and Testing

## Overview

This is the final phase that wires all components together, verifies end-to-end functionality, and adds comprehensive tests. The goal is to ensure all 20 API methods work correctly, all 11 event types are emitted properly, and the system is production-ready.

**Scope:**
- Final integration verification
- Unit tests for database operations
- Unit tests for subscription manager
- Unit tests for plugin registry
- Integration tests for WebSocket protocol
- End-to-end test scenarios
- Documentation updates
- Memory file updates

**Out of scope:**
- New features (all features should be implemented in Phases 1-5)
- Performance optimization (future work)
- Production deployment (future work)

## Dependencies

- **Phase 1: Database Extensions** — Must be completed
- **Phase 2: WebSocket Server Core** — Must be completed
- **Phase 3: Request Handlers** — Must be completed
- **Phase 4: Subscription System** — Must be completed
- **Phase 5: Plugin Management System** — Must be completed

## Integration Checklist

### 1. Module Structure Verification

Verify all modules are properly declared in `main.rs`:

```rust
mod config;
mod connection;
mod custom_events;
mod error;
mod events;
mod handlers;
mod plugins;
mod server;
mod subscriptions;
```

### 2. Handler Router Verification

Verify `handlers/mod.rs` routes all 20 methods:

**Chat methods (5):**
- [ ] `createChat` → `chat::create_chat`
- [ ] `listChats` → `chat::list_chats`
- [ ] `getChat` → `chat::get_chat`
- [ ] `deleteChat` → `chat::delete_chat`
- [ ] `updateChat` → `chat::update_chat`

**Message methods (3):**
- [ ] `addMessage` → `message::add_message`
- [ ] `updateMessage` → `message::update_message`
- [ ] `deleteMessage` → `message::delete_message`

**Subscription methods (4):**
- [ ] `subscribeChat` → `subscription::subscribe_chat`
- [ ] `unsubscribeChat` → `subscription::unsubscribe_chat`
- [ ] `subscribeChatsList` → `subscription::subscribe_chats_list`
- [ ] `unsubscribeChatsList` → `subscription::unsubscribe_chats_list`

**Plugin methods (8):**
- [ ] `registerPlugin` → `plugin::register_plugin`
- [ ] `getPlugins` → `plugin::get_plugins`
- [ ] `subscribePluginsList` → `plugin::subscribe_plugins_list`
- [ ] `unsubscribePluginsList` → `plugin::unsubscribe_plugins_list`
- [ ] `removePlugin` → `plugin::remove_plugin`
- [ ] `sendCustomEvent` → `plugin::send_custom_event`
- [ ] `ackCustomEvent` → `plugin::ack_custom_event`
- [ ] `getPendingAcks` → `plugin::get_pending_acks`

### 3. Event Broadcasting Verification

Verify all 11 event types are emitted:

**Chat events (3):**
- [ ] `chatCreated` — emitted by `createChat`
- [ ] `chatUpdated` — emitted by `updateChat`
- [ ] `chatDeleted` — emitted by `deleteChat`

**Message events (3):**
- [ ] `messageAdded` — emitted by `addMessage`
- [ ] `messageUpdated` — emitted by `updateMessage`
- [ ] `messageDeleted` — emitted by `deleteMessage`

**Plugin events (5):**
- [ ] `pluginRegistered` — emitted by `registerPlugin`
- [ ] `pluginRemoved` — emitted by `removePlugin`
- [ ] `pluginUpdated` — emitted on disconnect (when plugins are deactivated)
- [ ] `customEvent` — emitted by `sendCustomEvent`
- [ ] `customEventAcknowledged` — emitted by `ackCustomEvent`

### 4. Connection Lifecycle Verification

Verify connection lifecycle:
- [ ] Connection registration on connect
- [ ] Subscription cleanup on disconnect
- [ ] Plugin deactivation on disconnect
- [ ] Event channel cleanup on disconnect

## Unit Tests

### 1. Database Tests (`packages/rhd_db/src/chat_db/tests.rs`)

Already covered in Phase 1. Verify tests exist for:
- [ ] `test_chat_tags` — tag operations for chats
- [ ] `test_message_tags` — tag operations for messages
- [ ] `test_plugins` — plugin registration and management
- [ ] `test_custom_events` — custom event creation and acknowledgment

### 2. Subscription Manager Tests (`packages/rhd_chat_server/src/subscriptions.rs`)

Add unit tests at the bottom of `subscriptions.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscription_manager_chat_subscription() {
        let mut manager = SubscriptionManager::new();
        
        // Register connection
        let (conn_id, mut receiver) = manager.register_connection();
        
        // Subscribe to chat
        manager.subscribe_chat(&conn_id, 1);
        
        // Broadcast event
        let event = Event::new("test", serde_json::json!({}));
        manager.broadcast_to_chat(1, event.clone());
        
        // Verify event received
        let received = receiver.recv().await.unwrap();
        assert_eq!(received.event, "test");
        
        // Unsubscribe
        manager.unsubscribe_chat(&conn_id, 1);
        manager.broadcast_to_chat(1, event);
        
        // Verify no event received (channel should be empty)
        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_subscription_manager_chats_list_subscription() {
        let mut manager = SubscriptionManager::new();
        
        let (conn_id, mut receiver) = manager.register_connection();
        manager.subscribe_chats_list(&conn_id);
        
        let event = Event::new("chatCreated", serde_json::json!({}));
        manager.broadcast_to_chats_list(event);
        
        let received = receiver.recv().await.unwrap();
        assert_eq!(received.event, "chatCreated");
    }

    #[tokio::test]
    async fn test_subscription_manager_plugins_list_subscription() {
        let mut manager = SubscriptionManager::new();
        
        let (conn_id, mut receiver) = manager.register_connection();
        manager.subscribe_plugins_list(&conn_id);
        
        let event = Event::new("pluginRegistered", serde_json::json!({}));
        manager.broadcast_to_plugins_list(event);
        
        let received = receiver.recv().await.unwrap();
        assert_eq!(received.event, "pluginRegistered");
    }

    #[tokio::test]
    async fn test_subscription_manager_broadcast_to_all() {
        let mut manager = SubscriptionManager::new();
        
        let (conn_id1, mut receiver1) = manager.register_connection();
        let (conn_id2, mut receiver2) = manager.register_connection();
        
        let event = Event::new("customEvent", serde_json::json!({}));
        manager.broadcast_to_all(event);
        
        // Both connections should receive the event
        let received1 = receiver1.recv().await.unwrap();
        let received2 = receiver2.recv().await.unwrap();
        assert_eq!(received1.event, "customEvent");
        assert_eq!(received2.event, "customEvent");
    }

    #[tokio::test]
    async fn test_subscription_manager_cleanup_on_disconnect() {
        let mut manager = SubscriptionManager::new();
        
        let (conn_id, _receiver) = manager.register_connection();
        manager.subscribe_chat(&conn_id, 1);
        manager.subscribe_chats_list(&conn_id);
        manager.subscribe_plugins_list(&conn_id);
        
        // Unregister
        manager.unregister_connection(&conn_id);
        
        // Verify all subscriptions are cleaned up
        assert!(manager.chat_subscribers.get(&1).is_none() || 
                !manager.chat_subscribers.get(&1).unwrap().contains(&conn_id));
        assert!(!manager.chats_list_subscribers.contains(&conn_id));
        assert!(!manager.plugins_list_subscribers.contains(&conn_id));
    }
}
```

### 3. Plugin Registry Tests (`packages/rhd_chat_server/src/plugins.rs`)

Add unit tests at the bottom of `plugins.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registry_register() {
        let mut registry = PluginRegistry::new();
        
        let is_new = registry.register_plugin("plugin-1", "conn-1");
        assert!(is_new);
        
        assert_eq!(registry.get_connection_for_plugin("plugin-1"), Some("conn-1"));
        assert_eq!(registry.get_plugins_for_connection("conn-1"), vec!["plugin-1".to_string()]);
    }

    #[test]
    fn test_plugin_registry_re_register() {
        let mut registry = PluginRegistry::new();
        
        registry.register_plugin("plugin-1", "conn-1");
        let is_new = registry.register_plugin("plugin-1", "conn-2");
        
        assert!(!is_new);
        assert_eq!(registry.get_connection_for_plugin("plugin-1"), Some("conn-2"));
    }

    #[test]
    fn test_plugin_registry_remove() {
        let mut registry = PluginRegistry::new();
        
        registry.register_plugin("plugin-1", "conn-1");
        registry.remove_plugin("plugin-1");
        
        assert_eq!(registry.get_connection_for_plugin("plugin-1"), None);
        assert!(registry.get_plugins_for_connection("conn-1").is_empty());
    }

    #[test]
    fn test_plugin_registry_remove_plugins_for_connection() {
        let mut registry = PluginRegistry::new();
        
        registry.register_plugin("plugin-1", "conn-1");
        registry.register_plugin("plugin-2", "conn-1");
        
        let removed = registry.remove_plugins_for_connection("conn-1");
        assert_eq!(removed.len(), 2);
        assert!(removed.contains(&"plugin-1".to_string()));
        assert!(removed.contains(&"plugin-2".to_string()));
        
        assert_eq!(registry.get_connection_for_plugin("plugin-1"), None);
        assert_eq!(registry.get_connection_for_plugin("plugin-2"), None);
    }

    #[test]
    fn test_plugin_registry_multiple_plugins_per_connection() {
        let mut registry = PluginRegistry::new();
        
        registry.register_plugin("plugin-1", "conn-1");
        registry.register_plugin("plugin-2", "conn-1");
        
        let plugins = registry.get_plugins_for_connection("conn-1");
        assert_eq!(plugins.len(), 2);
    }
}
```

### 4. Custom Events Tests (`packages/rhd_chat_server/src/custom_events.rs`)

Add unit tests at the bottom of `custom_events.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rhd_db::ChatDb;

    #[test]
    fn test_create_custom_event() {
        let db = ChatDb::new(":memory:").unwrap();
        
        let (event_id, event) = create_custom_event(
            &db,
            "test-event",
            Some("plugin-1"),
            Some("{\"key\": \"value\"}"),
        ).unwrap();
        
        assert!(!event_id.is_empty());
        assert_eq!(event.event, "customEvent");
        
        // Verify event was stored
        let stored = db.get_custom_event(&event_id).unwrap().unwrap();
        assert_eq!(stored.event_name, "test-event");
        assert_eq!(stored.sender_plugin_id, Some("plugin-1".to_string()));
    }

    #[test]
    fn test_ack_custom_event() {
        let db = ChatDb::new(":memory:").unwrap();
        db.register_plugin("sender").unwrap();
        db.register_plugin("receiver").unwrap();
        
        let (event_id, _) = create_custom_event(&db, "test-event", Some("sender"), None).unwrap();
        
        let ack_event = ack_custom_event(&db, &event_id, "receiver").unwrap();
        assert!(ack_event.is_some());
        assert_eq!(ack_event.unwrap().event, "customEventAcknowledged");
        
        // Verify acknowledgment was stored
        assert!(db.has_plugin_acked(&event_id, "receiver").unwrap());
    }

    #[test]
    fn test_get_pending_events() {
        let db = ChatDb::new(":memory:").unwrap();
        db.register_plugin("plugin-1").unwrap();
        
        create_custom_event(&db, "event-1", None, None).unwrap();
        create_custom_event(&db, "event-2", None, None).unwrap();
        
        let pending = get_pending_events(&db, "plugin-1").unwrap();
        assert_eq!(pending.len(), 2);
    }
}
```

## Integration Tests

### 1. WebSocket Protocol Tests

Create `packages/rhd_chat_server/tests/websocket_tests.rs`:

```rust
//! Integration tests for WebSocket protocol.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};

async fn start_test_server() -> (u16, tokio::task::JoinHandle<()>) {
    // Find available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Start server in background
    let handle = tokio::spawn(async move {
        // TODO: Implement test server startup
        // This would need to call server::run() with test config
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    (port, handle)
}

async fn connect_to_server(port: u16) -> impl SinkExt<Message> + StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> {
    let url = format!("ws://127.0.0.1:{}/", port);
    let (ws_stream, _) = connect_async(&url).await.unwrap();
    ws_stream
}

async fn send_request(ws: &mut impl SinkExt<Message> + Unpin, method: &str, params: Value) -> Value {
    let request = json!({
        "type": "request",
        "id": "test-id",
        "method": method,
        "params": params
    });
    ws.send(Message::Text(serde_json::to_string(&request).unwrap())).await.unwrap();
    
    // Wait for response
    let msg = timeout(Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap();
    match msg {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        _ => panic!("Expected text message"),
    }
}

#[tokio::test]
async fn test_create_chat() {
    let (port, _handle) = start_test_server().await;
    let mut ws = connect_to_server(port).await;
    
    let response = send_request(&mut ws, "createChat", json!({
        "title": "Test Chat",
        "tags": ["test"]
    })).await;
    
    assert_eq!(response["success"], true);
    assert!(response["data"]["chatId"].is_number());
}

#[tokio::test]
async fn test_list_chats() {
    let (port, _handle) = start_test_server().await;
    let mut ws = connect_to_server(port).await;
    
    // Create a chat first
    send_request(&mut ws, "createChat", json!({"title": "Test"})).await;
    
    // List chats
    let response = send_request(&mut ws, "listChats", json!({})).await;
    
    assert_eq!(response["success"], true);
    assert!(response["data"]["chats"].is_array());
    assert!(response["data"]["chats"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_get_chat() {
    let (port, _handle) = start_test_server().await;
    let mut ws = connect_to_server(port).await;
    
    // Create a chat
    let create_response = send_request(&mut ws, "createChat", json!({"title": "Test"})).await;
    let chat_id = create_response["data"]["chatId"].as_i64().unwrap();
    
    // Get chat
    let response = send_request(&mut ws, "getChat", json!({"chatId": chat_id})).await;
    
    assert_eq!(response["success"], true);
    assert_eq!(response["data"]["chat"]["id"], chat_id);
}

#[tokio::test]
async fn test_add_message() {
    let (port, _handle) = start_test_server().await;
    let mut ws = connect_to_server(port).await;
    
    // Create a chat
    let create_response = send_request(&mut ws, "createChat", json!({"title": "Test"})).await;
    let chat_id = create_response["data"]["chatId"].as_i64().unwrap();
    
    // Add message
    let response = send_request(&mut ws, "addMessage", json!({
        "chatId": chat_id,
        "role": "user",
        "content": "Hello",
        "tags": ["greeting"]
    })).await;
    
    assert_eq!(response["success"], true);
    assert!(response["data"]["messageId"].is_number());
}

#[tokio::test]
async fn test_subscription_chat_events() {
    let (port, _handle) = start_test_server().await;
    
    // Connection 1: subscribe to chat
    let mut ws1 = connect_to_server(port).await;
    let create_response = send_request(&mut ws1, "createChat", json!({"title": "Test"})).await;
    let chat_id = create_response["data"]["chatId"].as_i64().unwrap();
    
    send_request(&mut ws1, "subscribeChat", json!({"chatId": chat_id})).await;
    
    // Connection 2: add message
    let mut ws2 = connect_to_server(port).await;
    send_request(&mut ws2, "addMessage", json!({
        "chatId": chat_id,
        "role": "user",
        "content": "Test message"
    })).await;
    
    // Connection 1 should receive messageAdded event
    let msg = timeout(Duration::from_secs(5), ws1.next()).await.unwrap().unwrap().unwrap();
    match msg {
        Message::Text(text) => {
            let event: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(event["type"], "event");
            assert_eq!(event["event"], "messageAdded");
        }
        _ => panic!("Expected text message"),
    }
}

#[tokio::test]
async fn test_plugin_registration() {
    let (port, _handle) = start_test_server().await;
    let mut ws = connect_to_server(port).await;
    
    let response = send_request(&mut ws, "registerPlugin", json!({
        "pluginId": "test-plugin"
    })).await;
    
    assert_eq!(response["success"], true);
    
    // Get plugins
    let response = send_request(&mut ws, "getPlugins", json!({})).await;
    assert_eq!(response["success"], true);
    let plugins = response["data"]["plugins"].as_array().unwrap();
    assert!(plugins.iter().any(|p| p["pluginId"] == "test-plugin"));
}

#[tokio::test]
async fn test_custom_event_flow() {
    let (port, _handle) = start_test_server().await;
    
    // Connection 1: register plugin and send custom event
    let mut ws1 = connect_to_server(port).await;
    send_request(&mut ws1, "registerPlugin", json!({"pluginId": "sender"})).await;
    
    let send_response = send_request(&mut ws1, "sendCustomEvent", json!({
        "eventName": "test-event",
        "additional": "{\"key\": \"value\"}"
    })).await;
    let event_id = send_response["data"]["eventId"].as_str().unwrap().to_string();
    
    // Connection 2: register plugin and receive event
    let mut ws2 = connect_to_server(port).await;
    send_request(&mut ws2, "registerPlugin", json!({"pluginId": "receiver"})).await;
    
    // Connection 2 should receive customEvent
    let msg = timeout(Duration::from_secs(5), ws2.next()).await.unwrap().unwrap().unwrap();
    match msg {
        Message::Text(text) => {
            let event: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(event["type"], "event");
            assert_eq!(event["event"], "customEvent");
            assert_eq!(event["data"]["eventId"], event_id);
        }
        _ => panic!("Expected text message"),
    }
    
    // Connection 2: acknowledge event
    send_request(&mut ws2, "ackCustomEvent", json!({"eventId": event_id})).await;
    
    // Connection 1 should receive customEventAcknowledged
    let msg = timeout(Duration::from_secs(5), ws1.next()).await.unwrap().unwrap().unwrap();
    match msg {
        Message::Text(text) => {
            let event: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(event["type"], "event");
            assert_eq!(event["event"], "customEventAcknowledged");
            assert_eq!(event["data"]["eventId"], event_id);
        }
        _ => panic!("Expected text message"),
    }
}
```

## End-to-End Test Scenarios

### Scenario 1: Complete Chat Workflow

1. Create chat with tags
2. Add messages with tags
3. Update chat title and tags
4. Update message content and tags
5. Get chat and verify all data
6. Delete message
7. Delete chat
8. Verify chat is gone

### Scenario 2: Subscription Workflow

1. Connection A subscribes to chats list
2. Connection B creates chat → A receives `chatCreated`
3. Connection A subscribes to specific chat
4. Connection B adds message → A receives `messageAdded`
5. Connection B updates message → A receives `messageUpdated`
6. Connection B deletes message → A receives `messageDeleted`
7. Connection B updates chat → A receives `chatUpdated`
8. Connection B deletes chat → A receives `chatDeleted`

### Scenario 3: Plugin Workflow

1. Connection A registers plugin "plugin-a"
2. Connection B subscribes to plugins list → receives `pluginRegistered`
3. Connection A sends custom event
4. All connections receive `customEvent`
5. Connection B registers plugin "plugin-b"
6. Connection B acknowledges event
7. Connection A receives `customEventAcknowledged`
8. Connection A calls `getPendingAcks` → empty list
9. Connection A disconnects
10. Connection B receives `pluginUpdated` (plugin-a is now inactive)
11. Connection C removes plugin-a
12. Connection B receives `pluginRemoved`

## Documentation Updates

### 1. Update `memory/MEMORY.md`

Add `rhd_chat_server` to the list of packages being kept:

```markdown
**Packages being kept (active development):**
- `rhd_util` — Shared error types, utilities, env var substitution
- `rhd_ai` — OpenAI-compatible AI client
- `rhd_db` — SQLite database layer
- `rhd_fsm` — Finite state machine framework
- `rhd_mcp_client` — MCP protocol client for tool usage
- `rhd_chat_api` — API types for chat WebSocket protocol
- `rhd_chat_server` — WebSocket server for chat storage and management (NEW)
```

### 2. Update `plans/state/rhd-chat-server-state.md`

Update the state file to reflect completion:

```markdown
## Current Implementation Status

### ✅ Completed

All phases have been completed:
- Phase 1: Database Extensions ✅
- Phase 2: WebSocket Server Core ✅
- Phase 3: Request Handlers ✅
- Phase 4: Subscription System ✅
- Phase 5: Plugin Management System ✅
- Phase 6: Integration and Testing ✅

The `rhd_chat_server` package is fully implemented and tested.
```

## Running Tests

### Unit Tests

```bash
# Run all unit tests
cargo test --package rhd_chat_server

# Run specific test module
cargo test --package rhd_chat_server subscriptions::tests
cargo test --package rhd_chat_server plugins::tests
cargo test --package rhd_chat_server custom_events::tests

# Run database tests
cargo test --package rhd_db chat_db::tests
```

### Integration Tests

```bash
# Run integration tests
cargo test --package rhd_chat_server --test websocket_tests
```

### Manual Testing

```bash
# Start server
cargo run --bin rhd_chat_server -- --port 8080 --db-path ./test.db

# Connect with websocat
websocat ws://127.0.0.1:8080/

# Send test requests
{"type": "request", "id": "1", "method": "createChat", "params": {"title": "Test"}}
```

## Success Criteria

1. ✅ All 20 API methods work correctly
2. ✅ All 11 event types are emitted to subscribed clients
3. ✅ Tags can be assigned to chats and messages
4. ✅ Clients can subscribe to specific chats or chats list
5. ✅ Database schema extended with tag tables, plugins, and custom events tables
6. ✅ Plugins can be registered, listed, and removed
7. ✅ Custom events can be sent, broadcast, and acknowledged
8. ✅ No AI calls or MCP tool execution in the codebase
9. ✅ All unit tests pass
10. ✅ All integration tests pass
11. ✅ Documentation is updated

## Final Verification

Before marking this phase as complete:

1. Run `cargo build --release` — should compile without errors
2. Run `cargo test` — all tests should pass
3. Run `cargo clippy` — no warnings
4. Manual testing — all 20 methods work
5. Verify no AI/MCP code in `rhd_chat_server`
6. Update memory files

## Next Steps

After this phase is complete:

1. Update `plans/state/rhd-chat-server-state.md` to mark all phases as completed
2. Update `memory/MEMORY.md` to add `rhd_chat_server`
3. Consider creating user documentation for the WebSocket API
4. Consider performance testing and optimization
5. Consider production deployment planning
