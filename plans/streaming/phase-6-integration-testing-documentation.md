# Phase 6: Integration Testing & Documentation

## Overview

Verify end-to-end streaming works correctly through integration tests and update project documentation to reflect the new streaming architecture.

## Scope

- Add integration tests for streaming in `rhd_plugin_ai_completions`
- Add WebSocket tests for stream methods in `rhd_chat_server`
- Update `memory/features/chat.md` with streaming details
- Update `memory/protocols.md` with new stream methods and events
- Update `memory/architecture.md` if needed

## Dependencies

- Phase 4 (plugin streaming).
- Phase 5 (frontend streaming).

---

## Files to Modify/Create

### 1. `plugins/rhd_plugin_ai_completions/tests/integration_test.rs`

**Add streaming integration tests:**

```rust
use rhd_chat_api::{
    AddMessageParams, GetChatParams, StreamPushParams, StreamFinishParams,
    StreamSubscribeParams, Message,
};
use rhd_chat_client::ChatClient;

/// Test: Plugin creates streaming message, pushes deltas, finishes stream.
#[tokio::test]
async fn test_streaming_ai_request_flow() {
    // Setup: Start chat server and plugin
    let server = start_test_server().await;
    let client = ChatClient::connect(&server.url()).await.unwrap();

    // Create a chat
    let chat = client.create_chat("Test Chat".to_string()).await.unwrap();
    let chat_id = chat.id;

    // Subscribe to chat events
    client.subscribe_chat(chat_id).await.unwrap();

    // Add a user message
    client.add_message(AddMessageParams {
        chat_id,
        role: "user".to_string(),
        content: "Hello".to_string(),
        reasoning_content: None,
        tags: vec![],
        is_finished: true,
        is_streaming: false,
    }).await.unwrap();

    // Simulate plugin creating a streaming message
    let add_result = client.add_message(AddMessageParams {
        chat_id,
        role: "assistant".to_string(),
        content: String::new(),
        reasoning_content: None,
        tags: vec![],
        is_finished: false,
        is_streaming: true,
    }).await.unwrap();

    let message_id = add_result.message_id;

    // Verify message was created with streaming flags
    let chat_data = client.get_chat(GetChatParams { chat_id, if_version_higher_than: None }).await.unwrap();
    let streaming_msg = chat_data.messages.iter().find(|m| m.id == message_id).unwrap();
    assert!(!streaming_msg.is_finished);
    assert!(streaming_msg.is_streaming);

    // Subscribe to stream
    let sub_result = client.stream_subscribe(StreamSubscribeParams { chat_id }).await.unwrap();
    assert!(sub_result.content.is_empty());
    assert!(!sub_result.is_finished);

    // Push reasoning delta
    client.stream_push(StreamPushParams {
        chat_id,
        reasoning_content: Some("Let me think...".to_string()),
        content: None,
        tool_calls: None,
    }).await.unwrap();

    // Push content delta
    client.stream_push(StreamPushParams {
        chat_id,
        reasoning_content: None,
        content: Some("Hello world".to_string()),
        tool_calls: None,
    }).await.unwrap();

    // Finish stream
    client.stream_finish(StreamFinishParams {
        chat_id,
        reasoning_content: Some("Full reasoning".to_string()),
        content: Some("Full content".to_string()),
        tool_calls: None,
    }).await.unwrap();

    // Update message with final content
    client.update_message(rhd_chat_api::UpdateMessageParams {
        message_id,
        content: Some("Full content".to_string()),
        reasoning_content: Some("Full reasoning".to_string()),
        role: None,
        add_tags: vec![],
        remove_tags: vec![],
        is_finished: Some(true),
        is_streaming: Some(false),
        tool_calls: None,
    }).await.unwrap();

    // Verify final message state
    let chat_data = client.get_chat(GetChatParams { chat_id, if_version_higher_than: None }).await.unwrap();
    let final_msg = chat_data.messages.iter().find(|m| m.id == message_id).unwrap();
    assert!(final_msg.is_finished);
    assert!(!final_msg.is_streaming);
    assert_eq!(final_msg.content, "Full content");
    assert_eq!(final_msg.reasoning_content.as_deref(), Some("Full reasoning"));
}

/// Test: Stream subscription receives chunks in real-time.
#[tokio::test]
async fn test_stream_subscription_receives_chunks() {
    let server = start_test_server().await;
    let client = ChatClient::connect(&server.url()).await.unwrap();
    let chat = client.create_chat("Test".to_string()).await.unwrap();

    // Subscribe to stream before any pushes
    let sub_result = client.stream_subscribe(StreamSubscribeParams { chat_id: chat.id }).await.unwrap();
    assert!(sub_result.content.is_empty());

    // Push content in another task
    let client2 = client.clone();
    let chat_id = chat.id;
    tokio::spawn(async move {
        for i in 0..5 {
            client2.stream_push(StreamPushParams {
                chat_id,
                reasoning_content: None,
                content: Some(format!("chunk{} ", i)),
                tool_calls: None,
            }).await.unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        client2.stream_finish(StreamFinishParams {
            chat_id,
            reasoning_content: None,
            content: None,
            tool_calls: None,
        }).await.unwrap();
    });

    // Verify we can get updated state via subscribe
    tokio::time::sleep(Duration::from_millis(300)).await;
    let sub_result2 = client.stream_subscribe(StreamSubscribeParams { chat_id: chat.id }).await.unwrap();
    // Content should have accumulated
    assert!(!sub_result2.content.is_empty() || sub_result2.is_finished);
}

/// Test: Finishing a non-existent stream doesn't panic.
#[tokio::test]
async fn test_finish_nonexistent_stream() {
    let server = start_test_server().await;
    let client = ChatClient::connect(&server.url()).await.unwrap();

    // Finish a stream for a chat that has no active stream
    let result = client.stream_finish(StreamFinishParams {
        chat_id: 99999,
        reasoning_content: None,
        content: None,
        tool_calls: None,
    }).await;

    // Should succeed (idempotent)
    assert!(result.is_ok());
}
```

---

### 2. `packages/rhd_chat_server/tests/websocket_tests.rs`

**Add stream method tests:**

```rust
/// Test: streamPush creates stream and broadcasts to subscribers.
#[tokio::test]
async fn test_stream_push_broadcasts() {
    let (server, port) = start_test_server().await;

    // Connect two clients
    let mut client1 = connect_client(port).await;
    let mut client2 = connect_client(port).await;

    // Create a chat
    let chat_id = create_chat(&mut client1, "Test").await;

    // Both clients subscribe to chat
    subscribe_chat(&mut client1, chat_id).await;
    subscribe_chat(&mut client2, chat_id).await;

    // Client1 pushes to stream
    send_request(&mut client1, "streamPush", json!({
        "chatId": chat_id,
        "content": "Hello"
    })).await;

    // Client2 should receive streamChunk event
    let event = receive_event(&mut client2).await;
    assert_eq!(event["event"], "streamChunk");
    assert_eq!(event["data"]["chatId"], chat_id);
    assert_eq!(event["data"]["type"], "contentDelta");
    assert_eq!(event["data"]["content"], "Hello");
}

/// Test: streamSubscribe returns current state and subscribes to future chunks.
#[tokio::test]
async fn test_stream_subscribe_returns_state() {
    let (server, port) = start_test_server().await;
    let mut client = connect_client(port).await;
    let chat_id = create_chat(&mut client, "Test").await;

    // Push some content first
    send_request(&mut client, "streamPush", json!({
        "chatId": chat_id,
        "reasoningContent": "Thinking...",
        "content": "Hello"
    })).await;

    // Subscribe to stream
    let result = send_request(&mut client, "streamSubscribe", json!({
        "chatId": chat_id
    })).await;

    assert_eq!(result["data"]["reasoningContent"], "Thinking...");
    assert_eq!(result["data"]["content"], "Hello");
    assert_eq!(result["data"]["isFinished"], false);
}

/// Test: streamFinish broadcasts streamFinished event.
#[tokio::test]
async fn test_stream_finish_broadcasts() {
    let (server, port) = start_test_server().await;
    let mut client1 = connect_client(port).await;
    let mut client2 = connect_client(port).await;
    let chat_id = create_chat(&mut client1, "Test").await;

    subscribe_chat(&mut client2, chat_id).await;

    // Push some content
    send_request(&mut client1, "streamPush", json!({
        "chatId": chat_id,
        "content": "Hello"
    })).await;

    // Finish stream
    send_request(&mut client1, "streamFinish", json!({
        "chatId": chat_id
    })).await;

    // Client2 should receive streamFinished event
    let event = receive_event(&mut client2).await;
    assert_eq!(event["event"], "streamFinished");
    assert_eq!(event["data"]["chatId"], chat_id);
}

/// Test: Message with isStreaming flag is correctly persisted and retrieved.
#[tokio::test]
async fn test_message_streaming_flags_persisted() {
    let (server, port) = start_test_server().await;
    let mut client = connect_client(port).await;
    let chat_id = create_chat(&mut client, "Test").await;

    // Add message with streaming flags
    let result = send_request(&mut client, "addMessage", json!({
        "chatId": chat_id,
        "role": "assistant",
        "content": "",
        "isFinished": false,
        "isStreaming": true
    })).await;

    let message_id = result["data"]["messageId"].as_i64().unwrap();

    // Get chat and verify message flags
    let chat_result = send_request(&mut client, "getChat", json!({
        "chatId": chat_id
    })).await;

    let messages = chat_result["data"]["messages"].as_array().unwrap();
    let msg = messages.iter().find(|m| m["id"].as_i64().unwrap() == message_id).unwrap();
    assert_eq!(msg["isFinished"], false);
    assert_eq!(msg["isStreaming"], true);

    // Update message to finished
    send_request(&mut client, "updateMessage", json!({
        "messageId": message_id,
        "content": "Final content",
        "isFinished": true,
        "isStreaming": false
    })).await;

    // Verify updated flags
    let chat_result2 = send_request(&mut client, "getChat", json!({
        "chatId": chat_id
    })).await;

    let messages2 = chat_result2["data"]["messages"].as_array().unwrap();
    let msg2 = messages2.iter().find(|m| m["id"].as_i64().unwrap() == message_id).unwrap();
    assert_eq!(msg2["isFinished"], true);
    assert_eq!(msg2["isStreaming"], false);
    assert_eq!(msg2["content"], "Final content");
}
```

---

### 3. `memory/features/chat.md`

**Add streaming section:**

Add a new section after "Streaming Responses":

```markdown
### Server-Side Streaming (Plugin-Driven)

AI responses are streamed through the chat server via a plugin-driven streaming architecture:

1. **Stream Creation**: When `rhd_plugin_ai_completions` starts an AI request, it creates a message with `isStreaming: true, isFinished: false` before the request begins.
2. **Stream Push**: As the AI provider returns tokens, the plugin pushes deltas (reasoning content, content, tool calls) to the server-side stream via `streamPush`.
3. **Stream Subscription**: The frontend detects messages with `isStreaming: true` and calls `streamSubscribe` to receive the current accumulated content and future chunks.
4. **Stream Finish**: When the AI response is complete, the plugin calls `streamFinish` and updates the message with `isStreaming: false, isFinished: true` and the final content.

**Stream Events**:
- `streamChunk`: Broadcast to chat subscribers when new content is pushed. Contains `type` (reasoningDelta/contentDelta/toolCallDelta) and the delta content.
- `streamFinished`: Broadcast when a stream completes.

**Stream Methods**:
- `streamPush`: Called by plugins to push deltas to a stream.
- `streamSubscribe`: Called by frontend to get current state and subscribe to future chunks.
- `streamFinish`: Called by plugins to finalize a stream.

**Message Flags**:
- `isStreaming: true` — Message is currently being streamed.
- `isFinished: false` — Message content is not yet final.
- Default values: `isStreaming: false, isFinished: true` (backward compatible with non-streaming messages).
```

---

### 4. `memory/protocols.md`

**Add stream methods and events:**

Add to the "Client → Server requests" section:

```markdown
  - `streamPush`: Push streaming content deltas to an active stream. Params: `chatId`, `reasoningContent?`, `content?`, `toolCalls?`. Result: `{ success: true }`.
  - `streamSubscribe`: Subscribe to a stream and get current accumulated content. Params: `chatId`. Result: `{ reasoningContent, content, toolCalls, isFinished }`. Also subscribes the caller to `streamChunk` events.
  - `streamFinish`: Finish an active stream. Params: `chatId`, `reasoningContent?`, `content?`, `toolCalls?`. Result: `{ success: true }`.
```

Add to the "Server → Client events" section:

```markdown
  - `streamChunk`: Contains `chatId`, `type` (reasoningDelta/contentDelta/toolCallDelta), and delta content. Sent to chat subscribers when new content is pushed to a stream.
  - `streamFinished`: Contains `chatId`. Sent when a stream completes.
```

---

### 5. `memory/architecture.md`

**Add StreamManager to architecture:**

Add to the "rhd_chat_server" section:

```markdown
**StreamManager**: In-memory manager for active streams. Keyed by chat ID (1:1 relationship). Supports:
- `push(chat_id, reasoning_delta, content_delta, tool_calls_delta)` — Accumulate deltas and notify subscribers.
- `subscribe_and_get(chat_id)` — Atomically return current state and subscribe to future chunks.
- `finish(chat_id)` — Finalize stream, notify subscribers, and clean up.
```

---

## Tests

### Test Scenarios

1. **End-to-end streaming flow**:
   - Plugin creates streaming message → Frontend subscribes → Chunks arrive → Stream finishes → Message updated.

2. **Multiple subscribers**:
   - Two frontend clients subscribe to the same stream → Both receive all chunks.

3. **Late subscription**:
   - Stream has already received some chunks → New subscriber gets current state + future chunks.

4. **Error during streaming**:
   - AI request fails mid-stream → Stream is finished → Message updated with error content.

5. **Non-existent stream**:
   - `streamFinish` for a chat with no active stream → Succeeds (idempotent).

6. **Message flag persistence**:
   - Message created with `isStreaming: true` → Retrieved with correct flags → Updated to `isFinished: true` → Flags persist.

---

## Implementation Notes

1. **Test infrastructure**: The integration tests require a running chat server. Use the existing test utilities in `rhd_chat_server/tests/websocket_tests.rs` for server setup and client connection.

2. **Mock AI provider**: The `rhd_mock_ai_provider` can be used to simulate streaming AI responses for integration tests. Ensure the mock supports streaming mode.

3. **Documentation consistency**: Ensure all documentation files are updated consistently. The `memory/features/chat.md` describes the user-facing behavior, while `memory/protocols.md` describes the wire protocol.

4. **Backward compatibility**: Emphasize in documentation that `isFinished` defaults to `true` and `isStreaming` defaults to `false`, ensuring existing messages and API calls work without modification.
