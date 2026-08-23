# Error Handling Fix and Test Plan

## Problem Statement

From the logs provided, when an AI request fails:
1. The error tag `ai_completions:error` is added to the chat
2. But the exact error is NOT logged at ERROR level
3. The error message is NOT being added to the chat (only the tag appears)

## Root Cause Analysis

### Issue 1: Missing ERROR Level Logging
In [`plugins/rhd_plugin_ai_completions/src/ai_request.rs`](plugins/rhd_plugin_ai_completions/src/ai_request.rs:133), the error handling code adds a tag and attempts to add an error message, but there's no `tracing::error!` log statement to log the exact error at ERROR level.

### Issue 2: Error Message Not Being Added
Looking at the code at lines 146-156, it tries to add a message with role `"ai_completions:error"`. However, this role might not be valid or there might be validation issues. The message addition might be failing silently.

### Issue 3: No Error Support in Mock Provider
The `SimpleListener` in `packages/rhd_mock_ai_provider/src/listener.rs` has methods like `push_text`, `push_tool_call`, `push_stream_receiver`, but no method to push an error response. The `MockAiResponse` enum has an `Error` variant (based on server.rs), but there's no convenient method in `SimpleListener` to push errors.

## Proposed Changes

### 1. Add ERROR Level Logging in ai_request.rs
**File**: `plugins/rhd_plugin_ai_completions/src/ai_request.rs`

Add `tracing::error!` logging before adding the error tag and message:

```rust
Err(e) => {
    // Log the exact error at ERROR level
    tracing::error!(
        chat_id = chat_id,
        error = %e,
        "AI request failed"
    );

    // Add error tag to chat
    client
        .update_chat(UpdateChatParams {
            chat_id,
            title: None,
            add_tags: vec!["ai_completions:error".to_string()],
            remove_tags: vec![],
        })
        .await
        .map_err(|e| AiRequestError::TagAdd(e.to_string()))?;

    // Add error message
    let error_content = format!("AI request failed: {}", e);
    client
        .add_message(AddMessageParams {
            chat_id,
            role: "assistant".to_string(),  // Changed from "ai_completions:error"
            content: error_content,
            reasoning_content: None,
            tags: vec!["ai_completions:error".to_string()],
        })
        .await
        .map_err(|e| AiRequestError::MessageAdd(e.to_string()))?;

    Err(AiRequestError::AiRequest(e.to_string()))
}
```

**Key changes**:
- Add `tracing::error!` with chat_id and error details
- Change role from `"ai_completions:error"` to `"assistant"` (more standard)
- Add error tag to the message itself for traceability

### 2. Add push_error Method to SimpleListener
**File**: `packages/rhd_mock_ai_provider/src/listener.rs`

Add a method to push error responses:

```rust
/// Add an error response
pub fn push_error(&self, status: u16, message: impl Into<String>) {
    self.push_response(MockAiResponse::Error {
        status,
        message: message.into(),
    });
}
```

### 3. Write Integration Test for Error Handling
**File**: `plugins/rhd_plugin_ai_completions/tests/integration_test.rs`

Add a new test `test_plugin_handles_ai_error`:

```rust
#[tokio::test]
async fn test_plugin_handles_ai_error() {
    init_tracing();
    timeout(Duration::from_secs(15), async {
        // Create test environment with error response
        let chat_config = rhd_chat_server::config::Config {
            host: "127.0.0.1".to_string(),
            port: 0,
            db_path: ":memory:".to_string(),
        };
        let (chat_server_port, _server_handle) = rhd_chat_server::server::start(chat_config)
            .await
            .expect("Failed to start chat server");

        // Start mock AI provider with error response
        let listener = SimpleListener::new();
        listener.push_error(500, "Internal server error from mock AI");
        let mock_ai = MockAiProvider::start(listener)
            .await
            .expect("Failed to start mock AI provider");

        // Create credentials and config files...
        // (similar to TestEnv::new())

        // Start plugin in background
        let plugin_handle = tokio::spawn({
            let url = format!("ws://127.0.0.1:{}/", chat_server_port);
            let config = config.clone();
            async move {
                plugin::run_plugin(&url, "test_plugin", config).await
            }
        });

        // Wait for plugin to initialize
        sleep(Duration::from_millis(500)).await;

        // Connect client
        let client = ChatClient::connect(&format!("ws://127.0.0.1:{}/", chat_server_port))
            .await
            .expect("Failed to connect");

        // Create a chat
        let create_result = client
            .create_chat(CreateChatParams {
                title: "Error Test Chat".to_string(),
                tags: vec![],
            })
            .await
            .expect("Failed to create chat");

        // Add a queued message to trigger AI request
        client
            .add_queue_message(AddQueueMessageParams {
                chat_id: create_result.chat_id,
                role: "user".to_string(),
                content: "Hello".to_string(),
                reasoning_content: None,
                tags: vec![],
            })
            .await
            .expect("Failed to add queue message");

        // Wait for plugin to process and encounter error
        sleep(Duration::from_secs(3)).await;

        // Check that error tag was added to chat
        let chat_result = client
            .get_chat(GetChatParams {
                chat_id: create_result.chat_id,
            })
            .await
            .expect("Failed to get chat");

        // Verify error tag is present
        assert!(
            chat_result.chat.tags.contains(&"ai_completions:error".to_string()),
            "Chat should have error tag"
        );

        // Verify error message was added
        let error_messages: Vec<_> = chat_result
            .messages
            .iter()
            .filter(|m| m.tags.contains(&"ai_completions:error".to_string()))
            .collect();
        
        assert!(
            !error_messages.is_empty(),
            "Should have at least one error message"
        );

        // Verify error message content
        let error_msg = error_messages.first().unwrap();
        assert!(
            error_msg.content.contains("AI request failed"),
            "Error message should contain 'AI request failed'"
        );

        // Cleanup
        plugin_handle.abort();
    })
    .await
    .expect("Test timed out");
}
```

## Implementation Order

1. **Add `push_error` method to `SimpleListener`** in `packages/rhd_mock_ai_provider/src/listener.rs`
2. **Fix error handling in `ai_request.rs`** - add ERROR logging and fix message role
3. **Write integration test** in `plugins/rhd_plugin_ai_completions/tests/integration_test.rs`

## Testing Strategy

The integration test will:
1. Configure mock AI provider to return a 500 error
2. Trigger an AI request by adding a queued message
3. Verify that:
   - The error tag `ai_completions:error` is added to the chat
   - An error message is added to the chat with the error details
   - The error is logged at ERROR level (visible in test output with tracing enabled)

## Files to Modify

1. `packages/rhd_mock_ai_provider/src/listener.rs` - Add `push_error` method
2. `plugins/rhd_plugin_ai_completions/src/ai_request.rs` - Add ERROR logging and fix message role
3. `plugins/rhd_plugin_ai_completions/tests/integration_test.rs` - Add error handling test
