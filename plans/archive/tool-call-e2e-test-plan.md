# Tool Call E2E Test Plan

## Goal
Create an e2e test for the ai_completions plugin that verifies the complete tool call flow works correctly.

## Test Scenario
1. Create a chat
2. Register a tool (e.g., `get_weather`)
3. Send a user message
4. Plugin calls AI provider, which returns an assistant message with a tool call
5. Test provides the tool call result (adds a tool message)
6. Plugin detects tool loop continuation and calls AI provider again
7. AI provider returns an assistant message without tool calls
8. Test verifies the final state

## Critical Issue Found
In [`plugins/rhd_plugin_ai_completions/src/ai_request.rs:223`](plugins/rhd_plugin_ai_completions/src/ai_request.rs:223):
```rust
tools: None, // TODO: Add tools support
```

The plugin does NOT pass tools to the AI request. This must be fixed for the test to pass.

## Implementation Steps

### Step 1: Create the E2E Test
Create `plugins/rhd_plugin_ai_completions/tests/tool_call_e2e_test.rs` with:
- Test environment setup (chat server + mock AI provider with RecordingListener)
- Register a tool via `add_tools` API
- Queue a user message
- Wait for assistant message with tool call
- Add tool result message
- Wait for final assistant response
- Verify AI provider received 2 requests with correct data

### Step 2: Run Test and Confirm Failure
Run `cargo test -p rhd_plugin_ai_completions --test tool_call_e2e_test` to confirm the test fails because tools aren't passed.

### Step 3: Fix the Plugin
Modify `ai_request.rs` to:
1. Fetch tools for the chat using `get_tools` API
2. Convert `rhd_chat_api::ToolDefinition` to `rhd_ai_client::ToolDefinition`
3. Pass tools in the `ChatCompletionRequest`

### Step 4: Verify Test Passes
Run the test again to confirm it passes after the fix.

## Files to Modify
- `plugins/rhd_plugin_ai_completions/tests/tool_call_e2e_test.rs` (new)
- `plugins/rhd_plugin_ai_completions/src/ai_request.rs` (fix tools support)

## Success Criteria
- Test passes
- AI provider receives tools in the first request
- AI provider receives tool result in the second request
- Final assistant message is created without tool calls
