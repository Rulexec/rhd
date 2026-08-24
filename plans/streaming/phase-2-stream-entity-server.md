# Phase 2: Stream Entity in rhd_chat_server

## Overview

Implement the in-memory `StreamManager` in `rhd_chat_server` that holds active stream state per chat, supports push/subscribe/finish operations, and broadcasts stream chunks to subscribers via the existing subscription manager.

## Scope

- Create `StreamManager` with `HashMap<i64, StreamState>` keyed by chat ID
- Implement `push()`, `subscribe_and_get()`, and `finish()` methods
- Create stream handler module with `stream_push`, `stream_subscribe`, `stream_finish` handlers
- Wire stream manager into server initialization and connection handling
- Route new stream methods in the request handler

## Dependencies

- Phase 1 (needs the updated Message types with `is_finished`/`is_streaming` fields).

---

## Files to Create

### 1. `packages/rhd_chat_server/src/streams.rs` (NEW)

**StreamChunk enum:**

```rust
use serde::{Deserialize, Serialize};

/// A chunk of streaming content sent to subscribers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum StreamChunk {
    /// New reasoning/thinking content delta.
    ReasoningDelta { content: String },
    /// New main content delta.
    ContentDelta { content: String },
    /// Tool call delta (accumulated during streaming).
    ToolCallDelta { tool_calls: Vec<StreamToolCall> },
    /// Stream has finished. No more chunks will be sent.
    Finished,
}

/// A tool call accumulated during streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}
```

**StreamSnapshot struct:**

```rust
/// A snapshot of the current stream state, returned by subscribe_and_get.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSnapshot {
    pub chat_id: i64,
    pub reasoning_content: String,
    pub content: String,
    pub tool_calls: Vec<StreamToolCall>,
    pub is_finished: bool,
}
```

**StreamState struct (internal):**

```rust
use tokio::sync::mpsc;

struct StreamState {
    reasoning_content: String,
    content: String,
    tool_calls: Vec<StreamToolCall>,
    subscribers: Vec<mpsc::UnboundedSender<StreamChunk>>,
    is_finished: bool,
}
```

**StreamManager struct:**

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct StreamManager {
    streams: RwLock<HashMap<i64, StreamState>>,
}

pub type SharedStreamManager = Arc<StreamManager>;

impl StreamManager {
    pub fn new() -> Self {
        Self {
            streams: RwLock::new(HashMap::new()),
        }
    }

    /// Push deltas to a stream. Creates the stream if it doesn't exist.
    /// Notifies all subscribers with the new chunk.
    pub async fn push(
        &self,
        chat_id: i64,
        reasoning_delta: Option<String>,
        content_delta: Option<String>,
        tool_calls_delta: Option<Vec<StreamToolCall>>,
    ) {
        let mut streams = self.streams.write().await;
        let state = streams.entry(chat_id).or_insert_with(|| StreamState {
            reasoning_content: String::new(),
            content: String::new(),
            tool_calls: Vec::new(),
            subscribers: Vec::new(),
            is_finished: false,
        });

        if state.is_finished {
            return; // Ignore pushes to finished streams
        }

        // Accumulate content
        if let Some(delta) = &reasoning_delta {
            state.reasoning_content.push_str(delta);
        }
        if let Some(delta) = &content_delta {
            state.content.push_str(delta);
        }
        if let Some(delta) = &tool_calls_delta {
            // Merge tool calls by ID, or add new ones
            for new_call in delta {
                if let Some(existing) = state.tool_calls.iter_mut().find(|c| c.id == new_call.id) {
                    existing.arguments.push_str(&new_call.arguments);
                } else {
                    state.tool_calls.push(new_call.clone());
                }
            }
        }

        // Build chunk to send to subscribers
        let chunk = if let Some(delta) = reasoning_delta {
            Some(StreamChunk::ReasoningDelta { content: delta })
        } else if let Some(delta) = content_delta {
            Some(StreamChunk::ContentDelta { content: delta })
        } else if let Some(delta) = tool_calls_delta {
            Some(StreamChunk::ToolCallDelta { tool_calls: delta })
        } else {
            None
        };

        // Notify subscribers
        if let Some(chunk) = chunk {
            state.subscribers.retain(|sender| sender.send(chunk.clone()).is_ok());
        }
    }

    /// Atomically get current stream state AND subscribe to future chunks.
    /// Returns the snapshot and a receiver for future chunks.
    /// If no stream exists for this chat, returns a finished snapshot with empty content.
    pub async fn subscribe_and_get(
        &self,
        chat_id: i64,
    ) -> (StreamSnapshot, mpsc::UnboundedReceiver<StreamChunk>) {
        let mut streams = self.streams.write().await;
        let (sender, receiver) = mpsc::unbounded_channel();

        let state = streams.entry(chat_id).or_insert_with(|| StreamState {
            reasoning_content: String::new(),
            content: String::new(),
            tool_calls: Vec::new(),
            subscribers: Vec::new(),
            is_finished: false,
        });

        let snapshot = StreamSnapshot {
            chat_id,
            reasoning_content: state.reasoning_content.clone(),
            content: state.content.clone(),
            tool_calls: state.tool_calls.clone(),
            is_finished: state.is_finished,
        };

        // Only subscribe if stream is not finished
        if !state.is_finished {
            state.subscribers.push(sender);
        } else {
            // Stream already finished, send Finished chunk immediately
            let _ = sender.send(StreamChunk::Finished);
            // Drop sender so receiver gets the Finished message then closes
        }

        (snapshot, receiver)
    }

    /// Finish a stream. Returns the final snapshot.
    /// Sends Finished chunk to all subscribers and clears them.
    /// The stream entry is removed after finishing.
    pub async fn finish(&self, chat_id: i64) -> StreamSnapshot {
        let mut streams = self.streams.write().await;

        let state = match streams.remove(&chat_id) {
            Some(state) => state,
            None => {
                // No stream exists — return empty finished snapshot
                return StreamSnapshot {
                    chat_id,
                    reasoning_content: String::new(),
                    content: String::new(),
                    tool_calls: Vec::new(),
                    is_finished: true,
                };
            }
        };

        let snapshot = StreamSnapshot {
            chat_id,
            reasoning_content: state.reasoning_content.clone(),
            content: state.content.clone(),
            tool_calls: state.tool_calls.clone(),
            is_finished: true,
        };

        // Notify all subscribers that stream is finished
        for sender in &state.subscribers {
            let _ = sender.send(StreamChunk::Finished);
        }
        // Dropping state drops all senders, closing the channels

        snapshot
    }

    /// Check if a stream exists and is active (not finished) for a chat.
    pub async fn is_active(&self, chat_id: i64) -> bool {
        let streams = self.streams.read().await;
        streams.get(&chat_id).map_or(false, |s| !s.is_finished)
    }
}
```

---

### 2. `packages/rhd_chat_server/src/handlers/stream.rs` (NEW)

```rust
//! Stream operation handlers.

use serde_json::Value;
use tokio::sync::mpsc;

use rhd_chat_api::methods::{
    StreamFinishParams, StreamFinishResult, StreamPushParams, StreamPushResult,
    StreamSubscribeParams, StreamSubscribeResult,
};
use rhd_chat_api::protocol::Response;
use rhd_chat_api::ErrorResponse;

use crate::error::ServerError;
use crate::streams::{SharedStreamManager, StreamChunk};
use crate::subscriptions::SharedSubscriptionManager;

/// Handle `streamPush` request.
pub async fn stream_push(
    params: Value,
    request_id: &str,
    stream_manager: &SharedStreamManager,
    subscription_manager: &SharedSubscriptionManager,
    chat_id: i64,
) -> Result<Value, ServerError> {
    let params: StreamPushParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Convert tool call deltas
    let tool_calls_delta = params.tool_calls.map(|calls| {
        calls
            .into_iter()
            .map(|tc| crate::streams::StreamToolCall {
                id: tc.id,
                name: tc.name,
                arguments: tc.arguments,
            })
            .collect()
    });

    // Push to stream
    stream_manager
        .push(chat_id, params.reasoning_content, params.content, tool_calls_delta)
        .await;

    // Broadcast streamChunk event to chat subscribers
    let manager = subscription_manager.read().await;
    if let Some(reasoning) = &params.reasoning_content {
        let event = rhd_chat_api::protocol::Event::new(
            "streamChunk",
            serde_json::json!({
                "chatId": chat_id,
                "type": "reasoningDelta",
                "content": reasoning,
            }),
        );
        manager.broadcast_to_chat(chat_id, event);
    }
    if let Some(content) = &params.content {
        let event = rhd_chat_api::protocol::Event::new(
            "streamChunk",
            serde_json::json!({
                "chatId": chat_id,
                "type": "contentDelta",
                "content": content,
            }),
        );
        manager.broadcast_to_chat(chat_id, event);
    }
    if let Some(tool_calls) = &params.tool_calls {
        let event = rhd_chat_api::protocol::Event::new(
            "streamChunk",
            serde_json::json!({
                "chatId": chat_id,
                "type": "toolCallDelta",
                "toolCalls": tool_calls,
            }),
        );
        manager.broadcast_to_chat(chat_id, event);
    }

    let result = StreamPushResult { success: true };
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}

/// Handle `streamSubscribe` request.
pub async fn stream_subscribe(
    params: Value,
    request_id: &str,
    connection_id: &str,
    stream_manager: &SharedStreamManager,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: StreamSubscribeParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    let (snapshot, mut receiver) = stream_manager.subscribe_and_get(params.chat_id).await;

    // Spawn task to forward stream chunks to the connection
    let sub_manager = subscription_manager.clone();
    let conn_id = connection_id.to_string();
    let chat_id = params.chat_id;
    tokio::spawn(async move {
        while let Some(chunk) = receiver.recv().await {
            let event = match chunk {
                StreamChunk::ReasoningDelta { content } => {
                    rhd_chat_api::protocol::Event::new(
                        "streamChunk",
                        serde_json::json!({
                            "chatId": chat_id,
                            "type": "reasoningDelta",
                            "content": content,
                        }),
                    )
                }
                StreamChunk::ContentDelta { content } => {
                    rhd_chat_api::protocol::Event::new(
                        "streamChunk",
                        serde_json::json!({
                            "chatId": chat_id,
                            "type": "contentDelta",
                            "content": content,
                        }),
                    )
                }
                StreamChunk::ToolCallDelta { tool_calls } => {
                    rhd_chat_api::protocol::Event::new(
                        "streamChunk",
                        serde_json::json!({
                            "chatId": chat_id,
                            "type": "toolCallDelta",
                            "toolCalls": tool_calls,
                        }),
                    )
                }
                StreamChunk::Finished => {
                    let manager = sub_manager.read().await;
                    let event = rhd_chat_api::protocol::Event::new(
                        "streamFinished",
                        serde_json::json!({ "chatId": chat_id }),
                    );
                    manager.send_to_connection(&conn_id, event);
                    break;
                }
            };
            let manager = sub_manager.read().await;
            manager.send_to_connection(&conn_id, event);
        }
    });

    let result = StreamSubscribeResult {
        reasoning_content: snapshot.reasoning_content,
        content: snapshot.content,
        tool_calls: snapshot.tool_calls,
        is_finished: snapshot.is_finished,
    };
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}

/// Handle `streamFinish` request.
pub async fn stream_finish(
    params: Value,
    request_id: &str,
    stream_manager: &SharedStreamManager,
    subscription_manager: &SharedSubscriptionManager,
    chat_id: i64,
) -> Result<Value, ServerError> {
    let params: StreamFinishParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    let _snapshot = stream_manager.finish(chat_id).await;

    // Broadcast streamFinished event to chat subscribers
    let manager = subscription_manager.read().await;
    let event = rhd_chat_api::protocol::Event::new(
        "streamFinished",
        serde_json::json!({ "chatId": chat_id }),
    );
    manager.broadcast_to_chat(chat_id, event);

    let result = StreamFinishResult { success: true };
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}
```

---

## Files to Modify

### 3. `packages/rhd_chat_server/src/lib.rs`

**Add module declaration:**

```rust
pub mod streams;
```

---

### 4. `packages/rhd_chat_server/src/server.rs`

**Modify `start` function:**

Add stream manager initialization:

```rust
use crate::streams::StreamManager;

pub async fn start(config: Config) -> Result<(u16, tokio::task::JoinHandle<()>), ServerError> {
    let db = Arc::new(ChatDb::new(&config.db_path)?);
    info!("Database initialized at {}", config.db_path);

    let subscription_manager = new_shared_subscription_manager();
    info!("Subscription manager initialized");

    let plugin_registry = new_shared_plugin_registry();
    info!("Plugin registry initialized");

    let stream_manager = Arc::new(StreamManager::new());
    info!("Stream manager initialized");

    let listener = TcpListener::bind(&config.socket_addr()).await?;
    let actual_port = listener.local_addr()?.port();
    info!("WebSocket server listening on ws://{}/ (port {})", config.socket_addr(), actual_port);

    let handle = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New connection from: {}", addr);
                    let db = Arc::clone(&db);
                    let subscription_manager = subscription_manager.clone();
                    let plugin_registry = plugin_registry.clone();
                    let stream_manager = stream_manager.clone();
                    tokio::spawn(async move {
                        match accept_async(stream).await {
                            Ok(ws_stream) => {
                                let (write, read) = ws_stream.split();
                                if let Err(e) = handle_connection(
                                    read, write, db, subscription_manager, plugin_registry, stream_manager,
                                ).await {
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
                Err(e) => {
                    error!("Accept error: {}", e);
                    break;
                }
            }
        }
    });

    Ok((actual_port, handle))
}
```

---

### 5. `packages/rhd_chat_server/src/connection.rs`

**Modify `handle_connection` signature (line 27):**

Add `stream_manager` parameter:

```rust
use crate::streams::SharedStreamManager;

pub async fn handle_connection(
    read: WsRead,
    mut write: WsWrite,
    db: Arc<ChatDb>,
    subscription_manager: SharedSubscriptionManager,
    plugin_registry: SharedPluginRegistry,
    stream_manager: SharedStreamManager,
) -> Result<(), ServerError> {
    // ... existing code ...

    // Pass stream_manager to process_messages
    let result = process_messages(
        read, db.clone(), subscription_manager.clone(),
        plugin_registry.clone(), stream_manager.clone(), &connection_id, outgoing_tx,
    ).await;

    // ... rest unchanged ...
}
```

**Modify `process_messages` signature (line 96):**

Add `stream_manager` parameter and pass it to `handle_request`:

```rust
async fn process_messages(
    mut read: WsRead,
    db: Arc<ChatDb>,
    subscription_manager: SharedSubscriptionManager,
    plugin_registry: SharedPluginRegistry,
    stream_manager: SharedStreamManager,
    connection_id: &str,
    _outgoing_tx: mpsc::UnboundedSender<String>,
) -> Result<(), ServerError> {
    // ... in the message loop:
    let response = handlers::handle_request(
        request, &db, connection_id,
        subscription_manager.clone(), plugin_registry.clone(), stream_manager.clone(),
    ).await;
}
```

---

### 6. `packages/rhd_chat_server/src/handlers/mod.rs`

**Add stream module:**

```rust
pub mod stream;
```

**Modify `handle_request` signature (line 21):**

Add `stream_manager` parameter:

```rust
use crate::streams::SharedStreamManager;

pub async fn handle_request(
    request: Request,
    db: &ChatDb,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
    plugin_registry: SharedPluginRegistry,
    stream_manager: SharedStreamManager,
) -> Result<Value, ServerError> {
    let request_id = request.id.clone();

    match request.method.as_str() {
        // ... existing methods ...

        // Stream methods
        "streamPush" => {
            // Extract chat_id from params for routing
            let chat_id = request.params.get("chatId")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            stream::stream_push(request.params, &request_id, &stream_manager, &subscription_manager, chat_id).await
        }
        "streamSubscribe" => stream::stream_subscribe(request.params, &request_id, connection_id, &stream_manager, &subscription_manager).await,
        "streamFinish" => {
            let chat_id = request.params.get("chatId")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            stream::stream_finish(request.params, &request_id, &stream_manager, &subscription_manager, chat_id).await
        }

        // ... rest unchanged ...
    }
}
```

---

## Tests

### Unit Tests in `streams.rs`

1. **`test_stream_push_and_subscribe`**:
   - Create a StreamManager.
   - Push reasoning delta, then content delta.
   - Call `subscribe_and_get` — verify snapshot contains accumulated content.
   - Push more content — verify receiver gets the new chunk.

2. **`test_stream_finish`**:
   - Create a stream, push some content.
   - Call `finish` — verify snapshot has `is_finished: true`.
   - Verify receiver gets `StreamChunk::Finished`.
   - Verify subsequent `push` is ignored.

3. **`test_stream_subscribe_after_finish`**:
   - Create a stream, finish it.
   - Call `subscribe_and_get` — verify snapshot has `is_finished: true` and receiver immediately gets `Finished`.

4. **`test_stream_multiple_subscribers`**:
   - Create a stream with 2 subscribers.
   - Push content — verify both receivers get the chunk.

5. **`test_stream_nonexistent_chat`**:
   - Call `subscribe_and_get` for a chat with no stream — verify empty finished snapshot.
   - Call `finish` for a chat with no stream — verify no panic, returns empty finished snapshot.

---

## Implementation Notes

1. **Atomic subscribe_and_get**: The `subscribe_and_get` method holds a write lock on the streams map while creating the subscription. This ensures no chunks can arrive between getting the snapshot and subscribing.

2. **Subscriber cleanup**: Subscribers are cleaned up lazily — when `send()` fails (receiver dropped), the sender is removed from the list via `retain()`.

3. **Stream lifecycle**: Streams are created implicitly on first `push` or `subscribe_and_get`. They are removed from the map on `finish`. This keeps memory usage minimal.

4. **Thread safety**: `StreamManager` uses `tokio::sync::RwLock` for async-safe concurrent access. The `SharedStreamManager` is `Arc<StreamManager>` (the `RwLock` is internal).

5. **Event broadcasting**: Stream chunks are broadcast to chat subscribers via the existing `SubscriptionManager::broadcast_to_chat`. The `streamSubscribe` handler also spawns a task to forward chunks directly to the subscribing connection via `send_to_connection`.
