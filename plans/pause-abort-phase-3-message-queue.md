# Phase 3: Message Queue

## Overview

This phase implements message queuing for when chat is paused or aborted, allowing users to add messages that will be sent on resume. Messages are stored in memory and processed when the chat resumes.

**Scope:**
- Add `MessageQueue` struct to store queued messages per chat
- Add `queue_message()` method to `ChatManager`
- Add `process_queued_messages()` method to `ChatManager`
- Add `QueueMessage` WebSocket request type
- Add `handle_queue_message()` handler
- Add `MessageQueued` event for frontend notification

**Out of scope:**
- Frontend UI changes (Phase 4)
- Test case creation (Phase 5)

**Dependencies:**
- Phase 1 (Backend State Machine) must be completed first
- Phase 2 (Tool Loop Integration) must be completed first

## Files to Modify

### 1. `packages/rhd_chat/src/state.rs`

**Add `QueuedMessage` struct:**

```rust
/// Represents a message queued while chat is paused/aborted
#[derive(Debug, Clone)]
pub struct QueuedMessage {
    pub content: String,
    pub model: String,
    pub queued_at: chrono::DateTime<chrono::Utc>,
}
```

**Add `MessageQueue` struct:**

```rust
/// Stores queued messages for a chat
#[derive(Debug, Clone, Default)]
pub struct MessageQueue {
    pub messages: Vec<QueuedMessage>,
}

impl MessageQueue {
    pub fn new() -> Self {
        Self { messages: Vec::new() }
    }

    pub fn push(&mut self, message: QueuedMessage) {
        self.messages.push(message);
    }

    pub fn drain(&mut self) -> Vec<QueuedMessage> {
        std::mem::take(&mut self.messages)
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }
}
```

**Update `StreamState` variants to include message queue:**

```rust
#[derive(Debug)]
pub enum StreamState {
    Running {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
        phase: ExecutionPhase,
    },
    Paused {
        cancel_token: CancellationToken,
        pause_notify: Arc<Notify>,
        phase: ExecutionPhase,
        pending_tool_calls: Vec<PendingToolCall>,
        message_queue: MessageQueue,
    },
    Aborted {
        cancel_token: CancellationToken,
        aborted_tool_ids: Vec<String>,
        message_queue: MessageQueue,
    },
}
```

**Update `StreamState` methods:**

```rust
impl StreamState {
    pub fn message_queue(&self) -> Option<&MessageQueue> {
        match self {
            StreamState::Paused { message_queue, .. } => Some(message_queue),
            StreamState::Aborted { message_queue, .. } => Some(message_queue),
            StreamState::Running { .. } => None,
        }
    }

    pub fn message_queue_mut(&mut self) -> Option<&mut MessageQueue> {
        match self {
            StreamState::Paused { message_queue, .. } => Some(message_queue),
            StreamState::Aborted { message_queue, .. } => Some(message_queue),
            StreamState::Running { .. } => None,
        }
    }
}
```

**Update `StreamStateInfo` to include message queue:**

```rust
#[derive(Debug, Clone)]
pub struct StreamStateInfo {
    pub is_running: bool,
    pub is_paused: bool,
    pub is_aborted: bool,
    pub phase: Option<ExecutionPhase>,
    pub pending_tool_calls: Vec<PendingToolCall>,
    pub aborted_tool_ids: Vec<String>,
    pub queued_messages: Vec<QueuedMessage>,
}

impl From<&StreamState> for StreamStateInfo {
    fn from(state: &StreamState) -> Self {
        match state {
            StreamState::Running { phase, .. } => StreamStateInfo {
                is_running: true,
                is_paused: false,
                is_aborted: false,
                phase: Some(phase.clone()),
                pending_tool_calls: Vec::new(),
                aborted_tool_ids: Vec::new(),
                queued_messages: Vec::new(),
            },
            StreamState::Paused { phase, pending_tool_calls, message_queue, .. } => StreamStateInfo {
                is_running: false,
                is_paused: true,
                is_aborted: false,
                phase: Some(phase.clone()),
                pending_tool_calls: pending_tool_calls.clone(),
                aborted_tool_ids: Vec::new(),
                queued_messages: message_queue.messages.clone(),
            },
            StreamState::Aborted { aborted_tool_ids, message_queue, .. } => StreamStateInfo {
                is_running: false,
                is_paused: false,
                is_aborted: true,
                phase: None,
                pending_tool_calls: Vec::new(),
                aborted_tool_ids: aborted_tool_ids.clone(),
                queued_messages: message_queue.messages.clone(),
            },
        }
    }
}
```

### 2. `packages/rhd_chat/src/manager.rs`

**Add `queue_message()` method:**

```rust
pub async fn queue_message(
    &self,
    chat_id: i64,
    content: String,
    model: String,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    let mut active = self.active_streams.lock().await;
    
    if let Some(state) = active.get_mut(&chat_id) {
        if let Some(queue) = state.message_queue_mut() {
            let queued_message = QueuedMessage {
                content: content.clone(),
                model: model.clone(),
                queued_at: chrono::Utc::now(),
            };
            queue.push(queued_message);
            
            // Emit MessageQueued event
            let _ = event_sender.send(ChatEvent::MessageQueued {
                chat_id,
                content,
                model,
            });
            
            Ok(())
        } else {
            Err(ChatError::InvalidState("Chat is not paused or aborted".to_string()))
        }
    } else {
        Err(ChatError::ChatNotFound)
    }
}
```

**Add `process_queued_messages()` method:**

```rust
pub async fn process_queued_messages(
    &self,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<Vec<(String, String)>, ChatError> {
    let mut active = self.active_streams.lock().await;
    
    if let Some(state) = active.get_mut(&chat_id) {
        if let Some(queue) = state.message_queue_mut() {
            let messages = queue.drain();
            let result: Vec<(String, String)> = messages
                .into_iter()
                .map(|m| (m.content, m.model))
                .collect();
            
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    } else {
        Err(ChatError::ChatNotFound)
    }
}
```

**Update `pause_chat()` to initialize message queue:**

```rust
pub async fn pause_chat(
    &self,
    chat_id: i64,
    pending_tool_calls: Vec<PendingToolCall>,
) -> bool {
    let mut active = self.active_streams.lock().await;
    if let Some(StreamState::Running { cancel_token, pause_notify, phase }) = active.get(&chat_id) {
        let cancel_token = cancel_token.clone();
        let pause_notify = pause_notify.clone();
        let phase = phase.clone();
        active.insert(
            chat_id,
            StreamState::Paused {
                cancel_token,
                pause_notify,
                phase,
                pending_tool_calls,
                message_queue: MessageQueue::new(),
            },
        );
        true
    } else {
        false
    }
}
```

**Update `abort_chat()` to initialize message queue:**

```rust
pub async fn abort_chat(&self, chat_id: i64, aborted_tool_ids: Vec<String>) -> bool {
    let mut active = self.active_streams.lock().await;
    if let Some(state) = active.get(&chat_id) {
        let cancel_token = state.cancel_token().clone();
        cancel_token.cancel();
        active.insert(
            chat_id,
            StreamState::Aborted {
                cancel_token,
                aborted_tool_ids,
                message_queue: MessageQueue::new(),
            },
        );
        true
    } else {
        false
    }
}
```

**Update `resume_chat()` to preserve message queue:**

```rust
pub async fn resume_chat(&self, chat_id: i64) -> Option<ResumeInfo> {
    let mut active = self.active_streams.lock().await;
    if let Some(StreamState::Paused { cancel_token, pause_notify, phase, pending_tool_calls, message_queue }) = active.get(&chat_id) {
        let cancel_token = cancel_token.clone();
        let pause_notify = pause_notify.clone();
        let pending_tool_calls = pending_tool_calls.clone();
        let message_queue = message_queue.clone();
        pause_notify.notify_one();
        active.insert(
            chat_id,
            StreamState::Running {
                cancel_token,
                pause_notify,
                phase: ExecutionPhase::AiCall,
            },
        );
        Some(ResumeInfo {
            pending_tool_calls,
            previous_phase: phase.clone(),
            message_queue,
        })
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct ResumeInfo {
    pub pending_tool_calls: Vec<PendingToolCall>,
    pub previous_phase: ExecutionPhase,
    pub message_queue: MessageQueue,
}
```

### 3. `packages/rhd_chat/src/event.rs`

**Add `MessageQueued` event:**

```rust
pub enum ChatEvent {
    // ... existing events
    
    /// Message was queued while chat is paused/aborted
    MessageQueued {
        chat_id: i64,
        content: String,
        model: String,
    },
    
    // ... other events
}
```

### 4. `packages/rhd_api/src/ws.rs`

**Add `QueueMessage` WebSocket request type:**

```rust
pub enum WsRequest {
    // ... existing requests
    
    #[serde(rename = "queueMessage", rename_all = "camelCase")]
    QueueMessage {
        id: String,
        chat_id: i64,
        content: String,
        model: String,
    },
    
    // ... other requests
}
```

### 5. `packages/rhd_app/src/ws/handlers/chat.rs`

**Add `handle_queue_message()` handler:**

```rust
pub async fn handle_queue_message(
    state: &DaemonState,
    id: String,
    chat_id: i64,
    content: String,
    model: String,
) -> WsResponse {
    match state.chat_manager.queue_message(chat_id, content, model, &state.chat_event_sender).await {
        Ok(()) => WsResponse::success(id, serde_json::json!({})),
        Err(e) => WsResponse::error(id, ErrorCode::InternalError, e.to_string()),
    }
}
```

**Update WebSocket handler to route `QueueMessage` requests:**

In the main WebSocket handler, add a case for `QueueMessage`:

```rust
WsRequest::QueueMessage { id, chat_id, content, model } => {
    handle_queue_message(&state, id, chat_id, content, model).await
}
```

### 6. `packages/rhd_chat/src/stream.rs`

**Update `send_message()` to process queued messages on resume:**

When resuming from pause/abort, process queued messages before starting the AI call:

```rust
pub async fn send_message<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    content: String,
    model: &str,
    models: &HashMap<String, ModelConfig>,
    event_sender: broadcast::Sender<ChatEvent>,
    reload_lock: &tokio::sync::RwLock<()>,
    template_loader: &TemplateLoaderRef,
) -> Result<i64, ChatError> {
    // ... existing code
    
    // Check if resuming from pause/abort
    if let Some(resume_info) = manager.resume_chat(chat_id).await {
        // Process queued messages
        let queued_messages = resume_info.message_queue.drain();
        
        // Add queued messages to chat history
        for queued_msg in queued_messages {
            manager.add_message_and_notify(
                chat_id,
                "user",
                &queued_msg.content,
                Some(&queued_msg.model),
                None,
                &event_sender,
            )?;
        }
        
        // Continue with tool loop using the queued messages
        // ... rest of the logic
    }
    
    // ... rest of existing code
}
```

**Update `resume_chat()` flow to handle pending tool calls and queued messages:**

When resuming, the flow should be:
1. Process pending tool calls (if any)
2. Execute tools and send results to AI
3. Process queued messages
4. Append queued messages to chat history
5. Send AI chat request with full context

```rust
pub async fn resume_and_continue<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    models: &HashMap<String, ModelConfig>,
    event_sender: broadcast::Sender<ChatEvent>,
    reload_lock: &tokio::sync::RwLock<()>,
    template_loader: &TemplateLoaderRef,
) -> Result<i64, ChatError> {
    // Get resume info
    let resume_info = manager.resume_chat(chat_id).await
        .ok_or_else(|| ChatError::InvalidState("Chat is not paused".to_string()))?;
    
    // Process pending tool calls
    if !resume_info.pending_tool_calls.is_empty() {
        // Execute pending tool calls
        for pending_tool in resume_info.pending_tool_calls {
            // Execute tool and save result
            // ... tool execution logic
        }
    }
    
    // Process queued messages
    let queued_messages = resume_info.message_queue.drain();
    for queued_msg in queued_messages {
        manager.add_message_and_notify(
            chat_id,
            "user",
            &queued_msg.content,
            Some(&queued_msg.model),
            None,
            &event_sender,
        )?;
    }
    
    // Continue tool loop
    // ... tool loop logic
    
    Ok(0) // Return message_id
}
```

### 7. `packages/rhd_chat/src/lib.rs`

**Export new types:**

```rust
pub use state::{ExecutionPhase, PendingToolCall, QueuedMessage, MessageQueue, StreamState, StreamStateInfo};
```

## Tests

### Unit Tests

Add to `packages/rhd_chat/src/state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_queue_push_and_drain() {
        let mut queue = MessageQueue::new();
        assert!(queue.is_empty());
        
        queue.push(QueuedMessage {
            content: "Hello".to_string(),
            model: "gpt-4".to_string(),
            queued_at: chrono::Utc::now(),
        });
        
        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());
        
        let messages = queue.drain();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello");
        assert!(queue.is_empty());
    }

    #[test]
    fn test_message_queue_multiple_messages() {
        let mut queue = MessageQueue::new();
        
        queue.push(QueuedMessage {
            content: "First".to_string(),
            model: "gpt-4".to_string(),
            queued_at: chrono::Utc::now(),
        });
        
        queue.push(QueuedMessage {
            content: "Second".to_string(),
            model: "gpt-4".to_string(),
            queued_at: chrono::Utc::now(),
        });
        
        assert_eq!(queue.len(), 2);
        
        let messages = queue.drain();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "First");
        assert_eq!(messages[1].content, "Second");
    }
}
```

Add to `packages/rhd_chat/src/manager.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_queue_message_when_paused() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider::new());
        let manager = ChatManager::new(db, project_provider, None, false);
        let (event_sender, _) = broadcast::channel(100);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (cancel_token, _) = manager.register_stream(chat_id).await;

        // Pause the chat
        manager.pause_chat(chat_id, Vec::new()).await;

        // Queue a message
        let result = manager.queue_message(
            chat_id,
            "Hello".to_string(),
            "gpt-4".to_string(),
            &event_sender,
        ).await;
        assert!(result.is_ok());

        // Verify message is queued
        let state = manager.get_stream_state(chat_id).await.unwrap();
        assert_eq!(state.queued_messages.len(), 1);
        assert_eq!(state.queued_messages[0].content, "Hello");
    }

    #[tokio::test]
    async fn test_queue_message_when_running_fails() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider::new());
        let manager = ChatManager::new(db, project_provider, None, false);
        let (event_sender, _) = broadcast::channel(100);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (cancel_token, _) = manager.register_stream(chat_id).await;

        // Try to queue a message while running
        let result = manager.queue_message(
            chat_id,
            "Hello".to_string(),
            "gpt-4".to_string(),
            &event_sender,
        ).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_process_queued_messages() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider::new());
        let manager = ChatManager::new(db, project_provider, None, false);
        let (event_sender, _) = broadcast::channel(100);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (cancel_token, _) = manager.register_stream(chat_id).await;

        // Pause the chat
        manager.pause_chat(chat_id, Vec::new()).await;

        // Queue multiple messages
        manager.queue_message(chat_id, "First".to_string(), "gpt-4".to_string(), &event_sender).await.unwrap();
        manager.queue_message(chat_id, "Second".to_string(), "gpt-4".to_string(), &event_sender).await.unwrap();

        // Process queued messages
        let messages = manager.process_queued_messages(chat_id, &event_sender).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].0, "First");
        assert_eq!(messages[1].0, "Second");

        // Verify queue is empty
        let state = manager.get_stream_state(chat_id).await.unwrap();
        assert_eq!(state.queued_messages.len(), 0);
    }

    #[tokio::test]
    async fn test_resume_preserves_message_queue() {
        let db = Arc::new(ChatDb::new(":memory:").unwrap());
        let project_provider = Arc::new(MockProjectProvider::new());
        let manager = ChatManager::new(db, project_provider, None, false);
        let (event_sender, _) = broadcast::channel(100);

        let chat_id = manager.create_chat("Test Chat").unwrap();
        let (cancel_token, _) = manager.register_stream(chat_id).await;

        // Pause the chat
        manager.pause_chat(chat_id, Vec::new()).await;

        // Queue a message
        manager.queue_message(chat_id, "Hello".to_string(), "gpt-4".to_string(), &event_sender).await.unwrap();

        // Resume the chat
        let resume_info = manager.resume_chat(chat_id).await.unwrap();
        assert_eq!(resume_info.message_queue.len(), 1);
        assert_eq!(resume_info.message_queue.messages[0].content, "Hello");
    }
}
```

### Integration Tests

Add to `packages/rhd_chat/tests/integration_tests.rs`:

```rust
#[tokio::test]
async fn test_message_queue_during_pause() {
    // Create chat
    // Start tool loop
    // Pause during AI call
    // Queue multiple messages
    // Verify messages are stored
    // Resume
    // Verify queued messages are processed
    // Verify messages are added to chat history
}

#[tokio::test]
async fn test_message_queue_during_abort() {
    // Create chat
    // Start tool loop
    // Abort during tool execution
    // Queue multiple messages
    // Verify messages are stored
    // Resume
    // Verify queued messages are processed
    // Verify messages are added to chat history
}
```

## Implementation Notes

1. **Queue storage**: Messages are stored in `MessageQueue` which is part of the `StreamState`. This ensures the queue is tied to the chat's lifecycle and is automatically cleaned up when the stream is unregistered.

2. **Queue processing**: On resume, queued messages are drained from the queue and added to the chat history before starting the AI call. This ensures the AI has full context including the queued messages.

3. **Frontend notification**: The `MessageQueued` event is emitted when a message is queued, allowing the frontend to show a "Queued" indicator for the message.

4. **Queue persistence**: Messages are NOT persisted to the database until they are processed on resume. This is intentional - queued messages are temporary and only exist in memory.

5. **Order preservation**: Messages are queued in order and processed in order (FIFO). The `Vec<QueuedMessage>` maintains insertion order.

6. **Model tracking**: Each queued message stores the model that was selected when the message was queued. This allows the user to queue messages with different models if needed.

7. **Error handling**: If `queue_message()` is called when the chat is not paused or aborted, it returns an error. This prevents messages from being queued when the chat is actively running.

8. **Thread safety**: All queue operations are protected by the `Mutex<HashMap<i64, StreamState>>` lock, ensuring thread-safe access.

## Dependencies

- This phase depends on: Phase 1 (Backend State Machine), Phase 2 (Tool Loop Integration)
- This phase must be completed before: Phase 4 (Frontend Integration)
- Phase 5 (Test Case Creation) can start after this phase
