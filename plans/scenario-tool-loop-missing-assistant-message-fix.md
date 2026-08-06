# Scenario Tool Loop Missing Assistant Message Fix

## Problem

The scenario tool loop in [`execute_with_tools()`](packages/rhd_app/src/scenario/ai_chat/mcp.rs:18) hangs when making AI requests with MCP tools. The root cause is that the message construction is missing the assistant message with tool calls.

### Current Behavior (Incorrect)

In [`mcp.rs:130-133`](packages/rhd_app/src/scenario/ai_chat/mcp.rs:130):
```rust
let mut messages = vec![ChatMessage::system(system_prompt), ChatMessage::user(&current_message)];
for (tool_call_id, content) in &tool_results {
    messages.push(ChatMessage::tool(tool_call_id, content));
}
```

This creates messages in the order: `[system, user, tool1, tool2, ...]`

### Expected Behavior (Correct)

According to the OpenAI API specification, tool messages must follow an assistant message that contains the corresponding tool calls. The correct order is:
`[system, user, assistant_with_tool_calls, tool1, tool2, ...]`

The chat tool loop in [`build_chat_messages_for_tools()`](packages/rhd_chat/src/tools/messages.rs:16) correctly handles this by storing all messages in the database, including the assistant message with tool calls.

## Solution

Modify the scenario tool loop to:
1. Store the assistant response (with tool calls) after each AI request
2. Include the assistant message with tool calls in the messages array before adding tool results

### Implementation Steps

1. **Modify [`execute_with_tools()`](packages/rhd_app/src/scenario/ai_chat/mcp.rs:18)**:
   - After receiving the AI response with tool calls, create an assistant message with those tool calls
   - Add this assistant message to the messages array before adding tool results
   - Use [`ChatMessage::assistant_with_tool_calls()`](packages/rhd_ai/src/client/types.rs:67) to create the message

2. **Update message construction logic**:
   - Change the message construction to include the assistant message with tool calls
   - The order should be: `[system, user, assistant_with_tool_calls, tool1, tool2, ...]`

### Code Changes

In [`mcp.rs`](packages/rhd_app/src/scenario/ai_chat/mcp.rs), modify the tool loop:

```rust
// After receiving AI response with tool calls (around line 199)
if result.tool_calls.is_empty() {
    // ... existing code for final response
}

// Store the assistant message with tool calls
let assistant_message = ChatMessage::assistant_with_tool_calls(
    result.content.clone(),
    None, // reasoning_content if available
    result.tool_calls.clone(),
);

// Update message construction (around line 130)
let mut messages = vec![ChatMessage::system(system_prompt), ChatMessage::user(&current_message)];
// Add assistant message with tool calls from previous iteration
if !tool_results.is_empty() {
    messages.push(assistant_message_from_previous_iteration);
}
for (tool_call_id, content) in &tool_results {
    messages.push(ChatMessage::tool(tool_call_id, content));
}
```

### Testing

1. Run the `example2` scenario that was hanging
2. Verify that the tool loop executes correctly
3. Verify that the final response is returned to the `rhd run` instance
4. Test with multiple tool calls to ensure the message order is correct

## Files to Modify

- [`packages/rhd_app/src/scenario/ai_chat/mcp.rs`](packages/rhd_app/src/scenario/ai_chat/mcp.rs) - Fix message construction in tool loop

## Related Files

- [`packages/rhd_ai/src/client/types.rs`](packages/rhd_ai/src/client/types.rs) - ChatMessage types
- [`packages/rhd_chat/src/tools/messages.rs`](packages/rhd_chat/src/tools/messages.rs) - Reference implementation for correct message construction
