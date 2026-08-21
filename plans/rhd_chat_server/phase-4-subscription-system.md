# Phase 4: Subscription System

## Overview

This phase implements the subscription system for real-time event broadcasting. Clients can subscribe to specific chats or the chats list, and receive events when changes occur. This phase covers 4 subscription methods and 6 event types: `chatCreated`, `chatUpdated`, `chatDeleted`, `messageAdded`, `messageUpdated`, and `messageDeleted`.

**Scope:**
- Create subscription manager to track subscriptions across connections
- Create event broadcasting system
- Implement 4 subscription handlers
- Integrate subscriptions into connection handler
- Modify Phase 3 handlers to emit events after operations
- Handle connection cleanup on disconnect

**Out of scope:**
- Plugin management (Phase 5)
- Custom events (Phase 5)

## Dependencies

- **Phase 1: Database Extensions** — Must be completed
- **Phase 2: WebSocket Server Core** — Must be completed
- **Phase 3: Request Handlers** — Must be completed (handlers will be modified to emit events)
- **rhd_chat_api** — Must be implemented for event types

## Architecture

### Subscription Manager

The `SubscriptionManager` is a shared state that tracks:
- Which connections are subscribed to which chat IDs
- Which connections are subscribed to the chats list
- A mapping from connection ID to a sender channel for pushing events

Each connection has a unique ID (UUID) and a `tokio::sync::mpsc` channel for receiving events.

### Event Flow

1. Client sends `subscribeChat` request
2. Handler adds connection ID to the chat's subscriber list
3. When a chat/message operation occurs, the handler calls `broadcast_event()`
4. `broadcast_event()` looks up all subscribers for the affected chat/chats list
5. Events are sent to each subscriber's channel
6. Connection handler reads from its channel and sends events to the WebSocket

### Connection Lifecycle

1. Connection is established → generate unique connection ID
2. Connection handler creates an event receiver channel
3. Register the connection with `SubscriptionManager`
4. Run two tasks:
   - Read task: reads WebSocket messages and routes to handlers
   - Write task: reads from event channel and sends to WebSocket
5. On disconnect → remove connection from `SubscriptionManager`

## Files to Create

### 1. `packages/rhd_chat_server/src/subscriptions.rs`

**Create subscription manager:**

```rust
//! Subscription manager for tracking client subscriptions and broadcasting events.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use rhd_chat_api::protocol::Event;

/// Unique identifier for a WebSocket connection.
pub type ConnectionId = String;

/// Channel for sending events to a connection.
pub type EventSender = mpsc::UnboundedSender<Event>;

/// Subscription manager that tracks client subscriptions.
#[derive(Default)]
pub struct SubscriptionManager {
    /// Map from connection ID to event sender channel.
    connections: HashMap<ConnectionId, EventSender>,
    /// Map from chat ID to set of subscribed connection IDs.
    chat_subscribers: HashMap<i64, HashSet<ConnectionId>>,
    /// Set of connection IDs subscribed to the chats list.
    chats_list_subscribers: HashSet<ConnectionId>,
}

impl SubscriptionManager {
    /// Create a new subscription manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new connection and return its ID and event receiver.
    pub fn register_connection(&mut self) -> (ConnectionId, mpsc::UnboundedReceiver<Event>) {
        let connection_id = Uuid::new_v4().to_string();
        let (sender, receiver) = mpsc::unbounded_channel();
        self.connections.insert(connection_id.clone(), sender);
        (connection_id, receiver)
    }

    /// Unregister a connection and remove all its subscriptions.
    pub fn unregister_connection(&mut self, connection_id: &str) {
        // Remove from chat subscriptions
        for subscribers in self.chat_subscribers.values_mut() {
            subscribers.remove(connection_id);
        }
        // Remove from chats list subscriptions
        self.chats_list_subscribers.remove(connection_id);
        // Remove connection
        self.connections.remove(connection_id);
    }

    /// Subscribe a connection to a specific chat.
    pub fn subscribe_chat(&mut self, connection_id: &str, chat_id: i64) {
        self.chat_subscribers
            .entry(chat_id)
            .or_insert_with(HashSet::new)
            .insert(connection_id.to_string());
    }

    /// Unsubscribe a connection from a specific chat.
    pub fn unsubscribe_chat(&mut self, connection_id: &str, chat_id: i64) {
        if let Some(subscribers) = self.chat_subscribers.get_mut(&chat_id) {
            subscribers.remove(connection_id);
            // Clean up empty sets
            if subscribers.is_empty() {
                self.chat_subscribers.remove(&chat_id);
            }
        }
    }

    /// Subscribe a connection to the chats list.
    pub fn subscribe_chats_list(&mut self, connection_id: &str) {
        self.chats_list_subscribers.insert(connection_id.to_string());
    }

    /// Unsubscribe a connection from the chats list.
    pub fn unsubscribe_chats_list(&mut self, connection_id: &str) {
        self.chats_list_subscribers.remove(connection_id);
    }

    /// Broadcast an event to all subscribers of a specific chat.
    pub fn broadcast_to_chat(&self, chat_id: i64, event: Event) {
        if let Some(subscribers) = self.chat_subscribers.get(&chat_id) {
            for connection_id in subscribers {
                if let Some(sender) = self.connections.get(connection_id) {
                    let _ = sender.send(event.clone());
                }
            }
        }
    }

    /// Broadcast an event to all subscribers of the chats list.
    pub fn broadcast_to_chats_list(&self, event: Event) {
        for connection_id in &self.chats_list_subscribers {
            if let Some(sender) = self.connections.get(connection_id) {
                let _ = sender.send(event.clone());
            }
        }
    }

    /// Broadcast an event to both chat subscribers and chats list subscribers.
    pub fn broadcast_to_chat_and_list(&self, chat_id: i64, event: Event) {
        self.broadcast_to_chat(chat_id, event.clone());
        self.broadcast_to_chats_list(event);
    }
}

/// Thread-safe wrapper for subscription manager.
pub type SharedSubscriptionManager = Arc<RwLock<SubscriptionManager>>;

/// Create a new shared subscription manager.
pub fn new_shared_subscription_manager() -> SharedSubscriptionManager {
    Arc::new(RwLock::new(SubscriptionManager::new()))
}
```

### 2. `packages/rhd_chat_server/src/events.rs`

**Create event broadcasting helpers:**

```rust
//! Event broadcasting helpers.

use serde_json::Value;

use rhd_chat_api::common::{Chat, ChatSummary, Message};
use rhd_chat_api::events::{
    ChatCreatedData, ChatDeletedData, ChatUpdatedData, MessageAddedData, MessageDeletedData,
    MessageUpdatedData,
};
use rhd_chat_api::protocol::Event;

/// Create a `chatCreated` event.
pub fn chat_created_event(chat: Chat) -> Event {
    let data = ChatCreatedData { chat };
    Event::new("chatCreated", serde_json::to_value(data).unwrap())
}

/// Create a `chatUpdated` event.
pub fn chat_updated_event(chat: Chat) -> Event {
    let data = ChatUpdatedData { chat };
    Event::new("chatUpdated", serde_json::to_value(data).unwrap())
}

/// Create a `chatDeleted` event.
pub fn chat_deleted_event(chat_id: i64) -> Event {
    let data = ChatDeletedData { chat_id };
    Event::new("chatDeleted", serde_json::to_value(data).unwrap())
}

/// Create a `messageAdded` event.
pub fn message_added_event(chat_id: i64, message: Message) -> Event {
    let data = MessageAddedData { chat_id, message };
    Event::new("messageAdded", serde_json::to_value(data).unwrap())
}

/// Create a `messageUpdated` event.
pub fn message_updated_event(chat_id: i64, message: Message) -> Event {
    let data = MessageUpdatedData { chat_id, message };
    Event::new("messageUpdated", serde_json::to_value(data).unwrap())
}

/// Create a `messageDeleted` event.
pub fn message_deleted_event(chat_id: i64, message_id: i64) -> Event {
    let data = MessageDeletedData { chat_id, message_id };
    Event::new("messageDeleted", serde_json::to_value(data).unwrap())
}
```

### 3. `packages/rhd_chat_server/src/handlers/subscription.rs`

**Create subscription handlers:**

```rust
//! Subscription operation handlers.

use serde_json::Value;

use rhd_chat_api::methods::{
    SubscribeChatParams, SubscribeChatResult, SubscribeChatsListParams, SubscribeChatsListResult,
    UnsubscribeChatParams, UnsubscribeChatResult, UnsubscribeChatsListParams,
    UnsubscribeChatsListResult,
};
use rhd_chat_api::protocol::Response;
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;
use crate::subscriptions::SharedSubscriptionManager;

/// Handle `subscribeChat` request.
pub async fn subscribe_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    let params: SubscribeChatParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
        }
    };

    // Check if chat exists
    if db.get_chat(params.chat_id)?.is_none() {
        return Ok(ErrorResponse::chat_not_found(request_id, params.chat_id));
    }

    // Subscribe
    let mut manager = subscription_manager.write().await;
    manager.subscribe_chat(connection_id, params.chat_id);

    let result = SubscribeChatResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `unsubscribeChat` request.
pub async fn unsubscribe_chat(
    params: Value,
    _db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    let params: UnsubscribeChatParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
        }
    };

    // Unsubscribe
    let mut manager = subscription_manager.write().await;
    manager.unsubscribe_chat(connection_id, params.chat_id);

    let result = UnsubscribeChatResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `subscribeChatsList` request.
pub async fn subscribe_chats_list(
    _params: Value,
    _db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    // Subscribe
    let mut manager = subscription_manager.write().await;
    manager.subscribe_chats_list(connection_id);

    let result = SubscribeChatsListResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `unsubscribeChatsList` request.
pub async fn unsubscribe_chats_list(
    _params: Value,
    _db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    // Unsubscribe
    let mut manager = subscription_manager.write().await;
    manager.unsubscribe_chats_list(connection_id);

    let result = UnsubscribeChatsListResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}
```

### 4. Update `packages/rhd_chat_server/src/handlers/mod.rs`

**Add subscription handlers to router:**

```rust
pub mod chat;
pub mod message;
pub mod subscription;

use serde_json::Value;

use rhd_chat_api::protocol::{Request, Response};
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;
use crate::subscriptions::SharedSubscriptionManager;

/// Route a request to the appropriate handler.
pub async fn handle_request(
    request: Request,
    db: &ChatDb,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
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
        
        // Unknown method
        _ => Ok(ErrorResponse::invalid_request(
            request_id,
            format!("Unknown method: {}", request.method),
        )),
    }
}
```

### 5. Update `packages/rhd_chat_server/src/handlers/chat.rs`

**Modify handlers to emit events:**

Add `subscription_manager` parameter and event broadcasting to `create_chat`, `delete_chat`, and `update_chat`:

```rust
use crate::events::{chat_created_event, chat_deleted_event, chat_updated_event};
use crate::subscriptions::SharedSubscriptionManager;

/// Handle `createChat` request.
pub async fn create_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    // ... existing code ...

    // Create chat
    let chat_id = db.create_chat(&params.title)?;

    // Set tags if provided
    if !params.tags.is_empty() {
        db.set_chat_tags(chat_id, &params.tags)?;
    }

    // Broadcast chatCreated event
    let chat_tags = db.get_chat_tags(chat_id)?;
    let chat_info = db.get_chat(chat_id)?.unwrap();
    let chat = convert_chat_info_to_api(chat_info, chat_tags)?;
    let event = chat_created_event(chat);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chats_list(event);

    // Return result
    let result = CreateChatResult { chat_id };
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `deleteChat` request.
pub async fn delete_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    // ... existing code ...

    // Broadcast chatDeleted event
    let event = chat_deleted_event(params.chat_id);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat_and_list(params.chat_id, event);

    // Delete chat
    db.delete_chat(params.chat_id)?;

    let result = DeleteChatResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `updateChat` request.
pub async fn update_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    // ... existing code ...

    // Touch chat to update updated_at
    db.touch_chat(params.chat_id)?;

    // Broadcast chatUpdated event
    let chat_tags = db.get_chat_tags(params.chat_id)?;
    let chat_info = db.get_chat(params.chat_id)?.unwrap();
    let chat = convert_chat_info_to_api(chat_info, chat_tags)?;
    let event = chat_updated_event(chat);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chats_list(event);

    let result = UpdateChatResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}
```

### 6. Update `packages/rhd_chat_server/src/handlers/message.rs`

**Modify handlers to emit events:**

Add `subscription_manager` parameter and event broadcasting to `add_message`, `update_message`, and `delete_message`:

```rust
use crate::events::{message_added_event, message_deleted_event, message_updated_event};
use crate::subscriptions::SharedSubscriptionManager;

/// Handle `addMessage` request.
pub async fn add_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    // ... existing code ...

    // Add message
    let message_id = db.add_message(...)?;

    // Set tags if provided
    if !params.tags.is_empty() {
        db.set_message_tags(message_id, &params.tags)?;
    }

    // Touch chat to update updated_at
    db.touch_chat(params.chat_id)?;

    // Broadcast messageAdded event
    let msg_tags = db.get_message_tags(message_id)?;
    let db_message = db.get_message(message_id)?.unwrap();
    let message = convert_message_to_api(db_message, msg_tags)?;
    let event = message_added_event(params.chat_id, message);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat(params.chat_id, event);

    let result = AddMessageResult { message_id };
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `updateMessage` request.
pub async fn update_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    // ... existing code ...

    // Touch chat to update updated_at
    db.touch_chat(message.chat_id)?;

    // Broadcast messageUpdated event
    let msg_tags = db.get_message_tags(params.message_id)?;
    let db_message = db.get_message(params.message_id)?.unwrap();
    let message = convert_message_to_api(db_message, msg_tags)?;
    let event = message_updated_event(message.chat_id, message);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat(message.chat_id, event);

    let result = UpdateMessageResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `deleteMessage` request.
pub async fn delete_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Response, ServerError> {
    // ... existing code ...

    // Broadcast messageDeleted event
    let event = message_deleted_event(message.chat_id, params.message_id);
    let manager = subscription_manager.read().await;
    manager.broadcast_to_chat(message.chat_id, event);

    // Delete message
    db.delete_message(params.message_id)?;

    let result = DeleteMessageResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}
```

### 7. Update `packages/rhd_chat_server/src/connection.rs`

**Integrate subscription manager and event broadcasting:**

```rust
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error};

use rhd_chat_api::protocol::{Request, Response};
use rhd_db::ChatDb;

use crate::error::ServerError;
use crate::handlers;
use crate::subscriptions::SharedSubscriptionManager;

type WsRead = futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>;
type WsWrite = futures_util::sink::SplitSink<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>, Message>;

/// Handle a single WebSocket connection.
pub async fn handle_connection(
    mut read: WsRead,
    mut write: WsWrite,
    db: Arc<ChatDb>,
    subscription_manager: SharedSubscriptionManager,
) -> Result<(), ServerError> {
    // Register connection
    let (connection_id, mut event_receiver) = {
        let mut manager = subscription_manager.write().await;
        manager.register_connection()
    };

    debug!("Connection registered: {}", connection_id);

    // Spawn task to forward events to WebSocket
    let write_connection_id = connection_id.clone();
    let write_task = tokio::spawn(async move {
        while let Some(event) = event_receiver.recv().await {
            let event_json = match serde_json::to_string(&event) {
                Ok(j) => j,
                Err(e) => {
                    error!("Failed to serialize event: {}", e);
                    continue;
                }
            };
            if let Err(e) = write.send(Message::Text(event_json)).await {
                error!("Failed to send event to connection {}: {}", write_connection_id, e);
                break;
            }
        }
    });

    // Process incoming messages
    let result = process_messages(read, write, db, subscription_manager, &connection_id).await;

    // Unregister connection
    {
        let mut manager = subscription_manager.write().await;
        manager.unregister_connection(&connection_id);
    }

    // Abort write task
    write_task.abort();

    debug!("Connection unregistered: {}", connection_id);

    result
}

async fn process_messages(
    mut read: WsRead,
    mut write: WsWrite,
    db: Arc<ChatDb>,
    subscription_manager: SharedSubscriptionManager,
    connection_id: &str,
) -> Result<(), ServerError> {
    while let Some(msg) = read.next().await {
        let msg = msg?;

        match msg {
            Message::Text(text) => {
                debug!("Received text message: {}", text);
                
                // Parse as JSON
                let json: Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Failed to parse JSON: {}", e);
                        let error_response = rhd_chat_api::ErrorResponse::invalid_request(
                            "unknown",
                            format!("Invalid JSON: {}", e),
                        );
                        write.send(Message::Text(serde_json::to_string(&error_response)?)).await?;
                        continue;
                    }
                };

                // Parse as request
                let request: Request = match serde_json::from_value(json) {
                    Ok(r) => r,
                    Err(e) => {
                        error!("Failed to parse request: {}", e);
                        let error_response = rhd_chat_api::ErrorResponse::invalid_request(
                            "unknown",
                            format!("Invalid request format: {}", e),
                        );
                        write.send(Message::Text(serde_json::to_string(&error_response)?)).await?;
                        continue;
                    }
                };

                debug!("Parsed request: method={}, id={}", request.method, request.id);

                // Route to appropriate handler
                let response = match handlers::handle_request(
                    request,
                    &db,
                    connection_id,
                    subscription_manager.clone(),
                ).await {
                    Ok(resp) => resp,
                    Err(e) => {
                        error!("Handler error: {}", e);
                        rhd_chat_api::ErrorResponse::internal_error(
                            "unknown",
                            format!("Internal error: {}", e),
                        )
                    }
                };

                write.send(Message::Text(serde_json::to_string(&response)?)).await?;
            }
            Message::Binary(_) => {
                debug!("Received binary message (ignoring)");
            }
            Message::Ping(data) => {
                debug!("Received ping");
                write.send(Message::Pong(data)).await?;
            }
            Message::Pong(_) => {
                debug!("Received pong");
            }
            Message::Close(frame) => {
                debug!("Received close frame: {:?}", frame);
                break;
            }
            Message::Frame(_) => {
                // Raw frame, not typically used
            }
        }
    }

    Ok(())
}
```

### 8. Update `packages/rhd_chat_server/src/server.rs`

**Pass subscription manager to connection handler:**

```rust
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tracing::{error, info};

use rhd_db::ChatDb;

use crate::config::Config;
use crate::connection::handle_connection;
use crate::error::ServerError;
use crate::subscriptions::new_shared_subscription_manager;

/// Run the WebSocket server.
pub async fn run(config: Config) -> Result<(), ServerError> {
    // Initialize database
    let db = Arc::new(ChatDb::new(&config.db_path)?);
    info!("Database initialized at {}", config.db_path);

    // Create subscription manager
    let subscription_manager = new_shared_subscription_manager();
    info!("Subscription manager initialized");

    // Bind TCP listener
    let listener = TcpListener::bind(&config.socket_addr()).await?;
    info!("WebSocket server listening on ws://{}/", config.socket_addr());

    // Accept connections
    loop {
        let (stream, addr) = listener.accept().await?;
        info!("New connection from: {}", addr);

        let db = Arc::clone(&db);
        let subscription_manager = subscription_manager.clone();
        tokio::spawn(async move {
            match accept_async(stream).await {
                Ok(ws_stream) => {
                    let (write, read) = ws_stream.split();
                    if let Err(e) = handle_connection(read, write, db, subscription_manager).await {
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

## Implementation Notes

1. **Connection ID**: Each connection gets a unique UUID. This is used to track subscriptions and route events.

2. **Event Channels**: Each connection has an `mpsc::unbounded_channel` for receiving events. The unbounded channel is used to avoid blocking event broadcasters.

3. **Write Task**: A separate task reads from the event channel and sends events to the WebSocket. This allows concurrent message processing and event broadcasting.

4. **Cleanup**: When a connection disconnects, it's removed from the subscription manager. All its subscriptions are automatically cleaned up.

5. **Event Broadcasting**: Events are cloned and sent to all subscribers. The `broadcast_to_chat_and_list()` method is used for events that should go to both chat subscribers and chats list subscribers (e.g., `chatDeleted`).

6. **Lock Contention**: The subscription manager uses `RwLock` to allow concurrent reads. Write locks are only held briefly during subscription changes.

7. **Error Handling**: If sending an event fails (e.g., connection closed), the error is logged but doesn't affect other subscribers.

## Testing

### Manual Testing

1. **Subscribe to chat:**
   ```json
   {"type": "request", "id": "1", "method": "subscribeChat", "params": {"chatId": 1}}
   ```
   Expected: Success response

2. **Add message (from another connection):**
   ```json
   {"type": "request", "id": "2", "method": "addMessage", "params": {"chatId": 1, "role": "user", "content": "Test"}}
   ```
   Expected: First connection receives `messageAdded` event

3. **Subscribe to chats list:**
   ```json
   {"type": "request", "id": "3", "method": "subscribeChatsList", "params": {}}
   ```
   Expected: Success response

4. **Create chat (from another connection):**
   ```json
   {"type": "request", "id": "4", "method": "createChat", "params": {"title": "New Chat"}}
   ```
   Expected: First connection receives `chatCreated` event

5. **Unsubscribe:**
   ```json
   {"type": "request", "id": "5", "method": "unsubscribeChat", "params": {"chatId": 1}}
   ```
   Expected: Success response, no more events for chat 1

### Connection Cleanup Test

1. Connect and subscribe to chat 1
2. Close connection
3. From another connection, add message to chat 1
4. Verify no errors occur (event is not sent to closed connection)

## Success Criteria

1. Clients can subscribe to specific chats and receive events
2. Clients can subscribe to chats list and receive events
3. Events are broadcast to all subscribed clients
4. Connection cleanup works correctly on disconnect
5. No memory leaks from abandoned subscriptions
6. Event broadcasting doesn't block message processing

## Next Steps

After this phase is complete, proceed to **Phase 5: Plugin Management System** to implement plugin registration and custom events.
