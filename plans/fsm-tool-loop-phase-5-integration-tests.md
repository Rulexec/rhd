# Phase 5: Integration Tests

## Overview

This phase adds comprehensive integration tests for the FSM-based tool loop, focusing on deterministic pause/resume/abort testing. The tests verify that the FSM wrapper behaves identically to the old implementation.

**Scope:**
- Test complete tool loop scenarios with mock AI and MCP clients
- Test pause during AI call
- Test pause during tool execution
- Test resume from pause
- Test abort during AI call
- Test abort during tool execution
- Test multiple tool calls
- Test DB synchronization
- Test event interception
- Achieve >80% test coverage for new code

**Out of Scope:**
- Helper FSM tests (Phase 6)
- Performance benchmarks

## Files to Create

### 1. `packages/rhd_chat/src/tools/tests/fsm_integration_tests.rs` (NEW FILE)

**Purpose:** Comprehensive integration tests for FSM-based tool loop.

**Complete Implementation:**

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rhd_ai::client::{OpenAiClient, ToolCall};
    use rhd_ai::ToolDefinition;
    use rhd_db::ChatDb;
    use rhd_fsm::{ToolLoopFsmEvent, ToolLoopListenerCallback};
    use rhd_mcp_client::client::McpClient;
    use rhd_mcp_client::ToolResult;
    use tokio::sync::{broadcast, Notify};
    use tokio_util::sync::CancellationToken;

    use crate::chat_log::ChatLoggers;
    use crate::error::ChatError;
    use crate::event::ChatEvent;
    use crate::manager::ChatManager;
    use crate::stream::TemplateLoaderRef;
    use crate::ProjectProvider;

    use super::super::db_sync_listener::create_db_sync_listener;
    use super::super::fsm_wrapper::FsmToolLoop;
    use super::super::tool_loop::ToolLoopResult;

    /// Test helper: Create a mock AI client that returns predefined responses
    struct MockAiClient {
        responses: Vec<MockAiResponse>,
        call_count: AtomicUsize,
    }

    struct MockAiResponse {
        content: Option<String>,
        thinking_content: Option<String>,
        tool_calls: Vec<ToolCall>,
        finish_reason: Option<String>,
    }

    /// Test helper: Create a mock MCP client
    struct MockMcpClient {
        tool_results: Vec<(String, ToolResult)>,
    }

    /// Test scenario: Simple AI response without tool calls
    #[tokio::test]
    async fn test_simple_ai_response() {
        // Setup: Create mock AI client that returns a simple text response
        // Expected: FSM completes with Completed state
        // Verify: Message is inserted into DB, StreamFinished event is emitted
    }

    /// Test scenario: AI response with single tool call
    #[tokio::test]
    async fn test_single_tool_call() {
        // Setup: Create mock AI client that returns a tool call
        // Setup: Create mock MCP client that returns a tool result
        // Expected: FSM executes tool, sends result back to AI, completes
        // Verify: Tool call events are emitted, tool result is in DB
    }

    /// Test scenario: AI response with multiple tool calls
    #[tokio::test]
    async fn test_multiple_tool_calls() {
        // Setup: Create mock AI client that returns multiple tool calls
        // Setup: Create mock MCP clients for each tool
        // Expected: FSM executes all tools, sends results back to AI, completes
        // Verify: All tool calls are executed, all results are in DB
    }

    /// Test scenario: Pause during AI call
    #[tokio::test]
    async fn test_pause_during_ai_call() {
        // Setup: Create mock AI client with slow response
        // Setup: Trigger pause after AI call starts
        // Expected: FSM transitions to Paused state
        // Verify: Paused action is emitted, no tool calls executed
        // Resume: Send resume signal
        // Expected: FSM resumes AI call and completes
    }

    /// Test scenario: Pause during tool execution
    #[tokio::test]
    async fn test_pause_during_tool_execution() {
        // Setup: Create mock AI client that returns tool call
        // Setup: Create mock MCP client with slow tool execution
        // Setup: Trigger pause after tool execution starts
        // Expected: FSM completes current tool, then transitions to Paused
        // Verify: Tool result is in DB, Paused action is emitted
        // Resume: Send resume signal
        // Expected: FSM sends tool result to AI and completes
    }

    /// Test scenario: Abort during AI call
    #[tokio::test]
    async fn test_abort_during_ai_call() {
        // Setup: Create mock AI client with slow response
        // Setup: Trigger abort after AI call starts
        // Expected: FSM transitions to Aborted state
        // Verify: Aborted action is emitted, StreamAborted event is sent
        // Verify: Error is returned
    }

    /// Test scenario: Abort during tool execution
    #[tokio::test]
    async fn test_abort_during_tool_execution() {
        // Setup: Create mock AI client that returns tool call
        // Setup: Create mock MCP client with slow tool execution
        // Setup: Trigger abort after tool execution starts
        // Expected: FSM aborts current tool, transitions to Aborted state
        // Verify: Tool result shows "Aborted", StreamAborted event is sent
    }

    /// Test scenario: Max iterations exceeded
    #[tokio::test]
    async fn test_max_iterations_exceeded() {
        // Setup: Create mock AI client that always returns tool calls
        // Setup: Set max_iterations to 3
        // Expected: FSM executes 3 iterations, then returns error
        // Verify: Error message contains "max tool iterations"
    }

    /// Test scenario: DB synchronization
    #[tokio::test]
    async fn test_db_synchronization() {
        // Setup: Create FSM with DB sync listener
        // Execute: Run through a complete tool loop
        // Verify: All messages are in DB
        // Verify: Message IDs match between FSM and DB
        // Verify: WebSocket events were emitted for each message
    }

    /// Test scenario: Event interception with propagate flag
    #[tokio::test]
    async fn test_event_interception() {
        // Setup: Create listener that intercepts ToolCallRequested
        // Setup: Listener sets propagate to false for specific tool
        // Expected: FSM skips ExecuteToolCall action for intercepted tool
        // Verify: Tool was not executed
        // Verify: FSM continues with other tool calls
    }

    /// Test scenario: Multiple listeners
    #[tokio::test]
    async fn test_multiple_listeners() {
        // Setup: Register multiple listeners
        // Execute: Run through tool loop
        // Verify: All listeners received all events
        // Verify: Events were received in order
    }

    /// Test scenario: Listener removal
    #[tokio::test]
    async fn test_listener_removal() {
        // Setup: Register listener, then remove it
        // Execute: Run through tool loop
        // Verify: Removed listener did not receive events
    }

    /// Test scenario: Error handling in tool execution
    #[tokio::test]
    async fn test_tool_execution_error() {
        // Setup: Create mock MCP client that returns error
        // Execute: Run tool loop
        // Expected: FSM continues with error result
        // Verify: Error is logged, tool result shows error
    }

    /// Test scenario: Unknown tool handling
    #[tokio::test]
    async fn test_unknown_tool() {
        // Setup: Create mock AI client that returns tool call for unknown tool
        // Execute: Run tool loop
        // Expected: FSM returns error result for unknown tool
        // Verify: Error message contains "unknown tool"
    }

    /// Test scenario: Empty AI response
    #[tokio::test]
    async fn test_empty_ai_response() {
        // Setup: Create mock AI client that returns empty content
        // Execute: Run tool loop
        // Expected: FSM completes with empty message
        // Verify: Message is in DB with empty content
    }

    /// Test scenario: Thinking content handling
    #[tokio::test]
    async fn test_thinking_content() {
        // Setup: Create mock AI client that returns thinking content
        // Execute: Run tool loop
        // Expected: Thinking content is preserved in message
        // Verify: Message in DB has thinking_content
    }

    /// Test scenario: Concurrent pause and abort
    #[tokio::test]
    async fn test_concurrent_pause_and_abort() {
        // Setup: Trigger both pause and abort simultaneously
        // Expected: Abort takes precedence
        // Verify: FSM transitions to Aborted state
    }

    /// Test scenario: Resume after abort (should fail)
    #[tokio::test]
    async fn test_resume_after_abort() {
        // Setup: Abort the FSM
        // Execute: Try to resume
        // Expected: Resume fails, FSM stays in Aborted state
    }

    /// Test scenario: Message ID counter initialization
    #[tokio::test]
    async fn test_message_id_counter_initialization() {
        // Setup: Create FSM with specific initial message ID
        // Execute: Generate messages
        // Verify: Message IDs start from initial value
    }

    /// Test scenario: Tool call ID generation
    #[tokio::test]
    async fn test_tool_call_id_generation() {
        // Setup: Create FSM
        // Execute: Request multiple tool call IDs
        // Verify: IDs are unique and sequential
    }
}
```

### 2. `packages/rhd_chat/src/tools/tests/mod.rs`

**Modifications:**

Add export for the new test module:

```rust
mod fsm_integration_tests;
```

## Test Infrastructure

### Mock AI Client

Create a mock AI client that can be configured to return predefined responses:

```rust
struct MockAiClient {
    responses: Vec<MockAiResponse>,
    call_count: AtomicUsize,
    response_delay: Option<Duration>,
}

impl MockAiClient {
    fn new(responses: Vec<MockAiResponse>) -> Self {
        Self {
            responses,
            call_count: AtomicUsize::new(0),
            response_delay: None,
        }
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.response_delay = Some(delay);
        self
    }
}
```

### Mock MCP Client

Create a mock MCP client that can be configured to return predefined tool results:

```rust
struct MockMcpClient {
    tool_results: HashMap<String, ToolResult>,
    call_delay: Option<Duration>,
}

impl MockMcpClient {
    fn new() -> Self {
        Self {
            tool_results: HashMap::new(),
            call_delay: None,
        }
    }

    fn add_tool_result(mut self, tool_name: &str, result: ToolResult) -> Self {
        self.tool_results.insert(tool_name.to_string(), result);
        self
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.call_delay = Some(delay);
        self
    }
}
```

### Test Database

Create a test database helper:

```rust
fn create_test_db() -> Arc<ChatDb> {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    Arc::new(ChatDb::new(db_path.to_str().unwrap()).unwrap())
}
```

### Test Chat Manager

Create a test ChatManager helper:

```rust
fn create_test_manager(db: Arc<ChatDb>) -> ChatManager<TestProjectProvider> {
    ChatManager::new(db, Arc::new(TestProjectProvider))
}
```

## Running Tests

```bash
cd packages/rhd_chat
cargo test tools::tests::fsm_integration_tests
```

For coverage:

```bash
cargo tarpaulin --packages rhd_chat --out Html
```

## Implementation Notes

1. **Deterministic Testing**: All tests use mock clients with predefined responses. No real AI or MCP calls are made.

2. **Timing Control**: Tests that involve pause/resume use `tokio::time::sleep` and `Notify` to control timing.

3. **Event Verification**: Tests verify that the correct events are emitted in the correct order.

4. **DB Verification**: Tests verify that the database contains the expected messages after the tool loop completes.

5. **Error Cases**: Tests cover error scenarios like tool execution failures, unknown tools, and max iterations exceeded.

6. **Concurrency**: Tests verify that concurrent pause and abort are handled correctly.

7. **Isolation**: Each test creates its own database and mock clients to ensure isolation.

## Dependencies

- This phase depends on Phase 4 (Replace tool_loop)
- This phase must be completed before Phase 6 (Helper FSMs)

## Success Criteria

- [ ] All pause/resume/abort scenarios tested deterministically
- [ ] Tests are fast and reliable (no flaky tests)
- [ ] Edge cases covered (empty responses, errors, unknown tools)
- [ ] Test coverage > 80% for new code
- [ ] Tests run in < 10 seconds total
- [ ] Code compiles without warnings
