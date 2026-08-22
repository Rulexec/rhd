# RHD Chat Client Implementation Plan

## Overview

Create a new `rhd_chat_client` package that provides a typed WebSocket client for connecting to `rhd_chat_server`. The client will depend only on `rhd_chat_api` (not on `rhd_chat_server`), making it reusable by any Rust client.

## Package Structure

```
packages/rhd_chat_client/
├── Cargo.toml
└── src/
    ├── lib.rs           # Public API exports
    ├── client.rs        # Main ChatClient struct
    ├── error.rs         # Client error types
    └── event_stream.rs  # Event subscription handling
```

## Dependencies

```toml
[package]
name = "rhd_chat_client"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { workspace = true }
tokio-tungstenite = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
futures-util = { workspace = true }
uuid = { version = "1", features = ["v4"] }
thiserror = { workspace = true }
tracing = { workspace = true }

rhd_chat_api = { path = "../rhd_chat_api" }
```

## Core Design

### ChatClient Struct

```rust
pub struct ChatClient {
    // WebSocket connection (split into read/write halves)
    write_tx: mpsc::UnboundedSender<String>,
    
    // Pending requests waiting for responses
    pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<Response>>>>,
    
    // Event subscribers
    event_subscribers: Arc<Mutex<Vec<EventSubscriber>>>,
    
    // Connection task handle
    _connection_task: JoinHandle<()>,
}
```

### Constructor

```rust
impl ChatClient {
    /// Connect to a chat server at the given WebSocket URL.
    ///
    /// # Arguments
    /// * `url` - WebSocket URL (e.g., "ws://127.0.0.1:8080/")
    ///
    /// The client does not automatically register as a plugin. If plugin
    /// functionality is needed, call `register_plugin()` after connecting.
    pub async fn connect(url: &str) -> Result<Self, ClientError>;
}
```

### Request Methods

Each method in `rhd_chat_api` gets a corresponding async method on the client:

| API Method | Client Method | Params Type | Result Type |
|------------|---------------|-------------|-------------|
| `createChat` | `create_chat()` | `CreateChatParams` | `CreateChatResult` |
| `listChats` | `list_chats()` | `ListChatsParams` | `ListChatsResult` |
| `getChat` | `get_chat()` | `GetChatParams` | `GetChatResult` |
| `updateChat` | `update_chat()` | `UpdateChatParams` | `UpdateChatResult` |
| `deleteChat` | `delete_chat()` | `DeleteChatParams` | `DeleteChatResult` |
| `addMessage` | `add_message()` | `AddMessageParams` | `AddMessageResult` |
| `updateMessage` | `update_message()` | `UpdateMessageParams` | `UpdateMessageResult` |
| `deleteMessage` | `delete_message()` | `DeleteMessageParams` | `DeleteMessageResult` |
| `subscribeChat` | `subscribe_chat()` | `SubscribeChatParams` | `SubscribeChatResult` |
| `unsubscribeChat` | `unsubscribe_chat()` | `UnsubscribeChatParams` | `UnsubscribeChatResult` |
| `subscribeChatsList` | `subscribe_chats_list()` | `SubscribeChatsListParams` | `SubscribeChatsListResult` |
| `unsubscribeChatsList` | `unsubscribe_chats_list()` | `UnsubscribeChatsListParams` | `UnsubscribeChatsListResult` |
| `registerPlugin` | `register_plugin()` | `RegisterPluginParams` | `RegisterPluginResult` |
| `removePlugin` | `remove_plugin()` | `RemovePluginParams` | `RemovePluginResult` |
| `getPlugins` | `get_plugins()` | `GetPluginsParams` | `GetPluginsResult` |
| `subscribePluginsList` | `subscribe_plugins_list()` | `SubscribePluginsListParams` | `SubscribePluginsListResult` |
| `unsubscribePluginsList` | `unsubscribe_plugins_list()` | `UnsubscribePluginsListParams` | `UnsubscribePluginsListResult` |
| `sendCustomEvent` | `send_custom_event()` | `SendCustomEventParams` | `SendCustomEventResult` |
| `ackCustomEvent` | `ack_custom_event()` | `AckCustomEventParams` | `AckCustomEventResult` |
| `getPendingAcks` | `get_pending_acks()` | `GetPendingAcksParams` | `GetPendingAcksResult` |

### Event Subscription Methods

```rust
/// Cancellation token for unsubscribing from events.
pub struct CancellationToken {
    cancel_tx: oneshot::Sender<()>,
}

impl ChatClient {
    /// Subscribe to chat events (messageAdded, messageUpdated, messageDeleted).
    /// The callback receives the parsed event data.
    pub fn on_chat_event<F, Fut>(
        &self,
        chat_id: i64,
        callback: F,
    ) -> CancellationToken
    where
        F: Fn(ChatEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send,
    {
        // ...
    }

    /// Subscribe to chats list events (chatCreated, chatUpdated, chatDeleted).
    pub fn on_chats_list_event<F, Fut>(
        &self,
        callback: F,
    ) -> CancellationToken
    where
        F: Fn(ChatsListEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send,
    {
        // ...
    }

    /// Subscribe to custom events (for plugins).
    pub fn on_custom_event<F, Fut>(
        &self,
        callback: F,
    ) -> CancellationToken
    where
        F: Fn(CustomEventData) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send,
    {
        // ...
    }

    /// Subscribe to custom event acknowledgments (for plugins).
    pub fn on_custom_event_acknowledged<F, Fut>(
        &self,
        callback: F,
    ) -> CancellationToken
    where
        F: Fn(CustomEventAcknowledgedData) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send,
    {
        // ...
    }

    /// Subscribe to plugin list events (pluginRegistered, pluginUpdated, pluginRemoved).
    pub fn on_plugins_list_event<F, Fut>(
        &self,
        callback: F,
    ) -> CancellationToken
    where
        F: Fn(PluginsListEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send,
    {
        // ...
    }
}
```

### Event Types

```rust
/// Events that can occur on a subscribed chat.
#[derive(Debug, Clone)]
pub enum ChatEvent {
    MessageAdded(MessageAddedData),
    MessageUpdated(MessageUpdatedData),
    MessageDeleted(MessageDeletedData),
}

/// Events that can occur on the chats list.
#[derive(Debug, Clone)]
pub enum ChatsListEvent {
    ChatCreated(ChatCreatedData),
    ChatUpdated(ChatUpdatedData),
    ChatDeleted(ChatDeletedData),
}

/// Events that can occur on the plugins list.
#[derive(Debug, Clone)]
pub enum PluginsListEvent {
    PluginRegistered(PluginRegisteredData),
    PluginUpdated(PluginUpdatedData),
    PluginRemoved(PluginRemovedData),
}
```

## Internal Architecture

### Connection Task

The client spawns a background task that:
1. Reads messages from the WebSocket
2. Routes responses to pending requests via `pending_requests` map
3. Routes events to registered subscribers via `event_subscribers`

### Request/Response Flow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Client    │────▶│  WebSocket  │────▶│   Server    │
│  Method     │     │   Write     │     │             │
└─────────────┘     └─────────────┘     └─────────────┘
       │                                         │
       │ 1. Generate UUID                        │
       │ 2. Create oneshot channel               │
       │ 3. Store sender in pending_requests     │
       │ 4. Send Request JSON                    │
       │                                         │
       │                    ┌─────────────┐      │
       │◀───────────────────│  WebSocket  │◀─────│
       │                    │   Read      │      │
       └────────────────────┤             │      │
                            └─────────────┘      │
       │ 5. Parse Response                       │
       │ 6. Lookup oneshot by request ID         │
       │ 7. Send response through channel        │
       │ 8. Method returns Result<T, ClientError>│
```

### Event Flow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Server    │────▶│  WebSocket  │────▶│   Client    │
│             │     │   Read      │     │  Event Loop │
└─────────────┘     └─────────────┘     └─────────────┘
                                              │
                                              │ 1. Parse Event
                                              │ 2. Match event type
                                              │ 3. Notify subscribers
                                              ▼
                                        ┌─────────────┐
                                        │ Subscribers │
                                        │  Callback   │
                                        └─────────────┘
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    
    #[error("Connection closed")]
    ConnectionClosed,
    
    #[error("Request timeout")]
    Timeout,
    
    #[error("Server error: {code} - {message}")]
    Server { code: String, message: String },
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
```

## Implementation Steps

### Phase 1: Package Setup
1. Create `packages/rhd_chat_client/` directory
2. Create `Cargo.toml` with dependencies
3. Add package to workspace `Cargo.toml`
4. Create `src/lib.rs` with module declarations

### Phase 2: Core Client
1. Implement `error.rs` with `ClientError` type
2. Implement `client.rs` with:
   - `ChatClient` struct
   - `connect()` constructor
   - Internal connection task
   - Request/response routing
3. Implement `event_stream.rs` with:
   - `CancellationToken` type
   - Event subscriber management
   - Event dispatching

### Phase 3: Request Methods
1. Implement all 20 request methods on `ChatClient`
2. Each method:
   - Creates a `Request` with unique ID
   - Sends via WebSocket
   - Awaits response via oneshot channel
   - Parses result or returns error

### Phase 4: Event Subscriptions
1. Implement event subscription methods
2. Implement `CancellationToken` for cleanup
3. Route incoming events to appropriate subscribers

### Phase 5: Test Refactoring
1. Update `packages/rhd_chat_server/tests/websocket_tests.rs`
2. Replace manual WebSocket handling with `ChatClient`
3. Verify all tests pass with the new client

## Usage Example

```rust
use rhd_chat_client::ChatClient;
use rhd_chat_api::{CreateChatParams, AddMessageParams, RegisterPluginParams};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the server
    let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
    
    // Optionally register as a plugin if needed
    client.register_plugin(RegisterPluginParams {
        plugin_id: "my-plugin".to_string(),
    }).await?;
    
    // Create a chat
    let result = client.create_chat(CreateChatParams {
        title: "My Chat".to_string(),
        tags: vec!["test".to_string()],
    }).await?;
    
    let chat_id = result.chat_id;
    
    // Subscribe to events
    let cancel_token = client.on_chat_event(chat_id, |event| async move {
        match event {
            ChatEvent::MessageAdded(data) => {
                println!("New message: {}", data.message.content);
            }
            _ => {}
        }
    });
    
    // Add a message
    client.add_message(AddMessageParams {
        chat_id,
        role: "user".to_string(),
        content: "Hello!".to_string(),
        reasoning_content: None,
        tags: vec![],
    }).await?;
    
    // Cancel subscription when done
    cancel_token.cancel();
    
    Ok(())
}
```

## Testing Strategy

1. **Unit Tests**: Test serialization/deserialization of client messages
2. **Integration Tests**: Use the refactored `websocket_tests.rs` to verify:
   - All request methods work correctly
   - Event subscriptions receive events
   - Cancellation tokens properly unsubscribe
   - Plugin registration and custom event flow

## Notes

- The client does NOT depend on `rhd_chat_server`, only on `rhd_chat_api`
- All methods are async and return `Result<T, ClientError>`
- Event callbacks are async functions, allowing for complex handling
- The connection task automatically cleans up when the client is dropped
- Cancellation tokens provide explicit control over subscription lifetime
