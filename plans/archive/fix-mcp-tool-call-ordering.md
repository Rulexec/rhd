# Fix MCP Tool Call Ordering Issue

## Status: ✅ Completed

## Problem Statement

When AI makes a tool call that fails and then retries, the frontend displays tool call results incorrectly:
- First assistant message shows wrong tool call arguments but success result from retry
- Second assistant message shows no tool call name/arguments
- Results appear "from the future" before they should

## Root Cause

**Event ordering mismatch between backend and frontend:**

Backend (`packages/rhd_chat/src/tools.rs`) sends events in this order:
1. `MessageAdded` (intermediate assistant with toolCalls in JSON)
2. `ToolCallStarted`
3. `ToolCallCompleted`
4. `MessageAdded` (tool result)

Frontend (`frontend/src/lib/chatWs.ts`) expects:
1. `ToolCallStarted` → create temp assistant message
2. `MessageAdded` (intermediate assistant) → replace temp with real message
3. `ToolCallCompleted` → update with result
4. `MessageAdded` (tool result) → merge result (no-op, already there)

**What actually happens:**
1. `MessageAdded` arrives first → `streamingMessageId` is null → adds real message with toolCalls parsed from JSON (status: 'completed', no result)
2. `ToolCallStarted` arrives → creates **duplicate** temp message with same toolCall (status: 'running')
3. `ToolCallCompleted` → updates temp message with result
4. `MessageAdded` (tool result) → searches ALL assistant messages for matching toolCallId → updates **both** messages

This creates duplicates and causes results to be applied to wrong messages.

## Solution

### Backend Fix: Send `ToolCallStarted` BEFORE `MessageAdded`

**File**: `packages/rhd_chat/src/tools.rs`

Change the event order in `tool_loop()`:

```rust
// BEFORE (current code):
// 1. Save intermediate message
// 2. Send MessageAdded (intermediate)
// 3. For each tool: send ToolCallStarted, execute, send ToolCallCompleted, save tool msg, send MessageAdded (tool)

// AFTER (fixed code):
// 1. For each tool: send ToolCallStarted FIRST
// 2. Save intermediate message
// 3. Send MessageAdded (intermediate) - this replaces the temp message created by ToolCallStarted
// 4. For each tool: execute, send ToolCallCompleted, save tool msg, send MessageAdded (tool)
```

**Implementation:**

```rust
// Emit ToolCallStarted events FIRST (before saving intermediate message)
// This ensures frontend creates temp message before MessageAdded arrives
for tool_call in &result.tool_calls {
    let mcp_id = extract_mcp_id_from_tool_name(&tool_call.function.name);
    
    if let Some(ref mut l) = loggers {
        l.chat_log.log_tool_call(&tool_call.function.name, &tool_call.id, &tool_call.function.arguments);
    }
    
    let _ = event_sender.send(ChatEvent::ToolCallStarted {
        chat_id,
        tool_call_id: tool_call.id.clone(),
        tool_name: tool_call.function.name.clone(),
        arguments: tool_call.function.arguments.clone(),
        mcp_id: mcp_id.clone(),
    });
}

// NOW save intermediate assistant message (frontend will replace temp message)
let intermediate_msg_id = manager.db().add_message(
    chat_id,
    "assistant",
    &assistant_msg_content,
    Some(model),
    thinking_option,
)?;
let intermediate_message = Message { /* ... */ };
let _ = event_sender.send(ChatEvent::MessageAdded {
    chat_id,
    message: intermediate_message,
});

// Execute tools and send completion events
for tool_call in &result.tool_calls {
    if cancel_token.is_cancelled() { /* ... */ }
    
    let (tool_result, _) = execute_tool_call(tool_call, mcp_clients).await;
    
    if let Some(ref mut l) = loggers {
        l.chat_log.log_tool_result(&tool_call.function.name, &tool_call.id, &tool_result);
    }
    
    let _ = event_sender.send(ChatEvent::ToolCallCompleted {
        chat_id,
        tool_call_id: tool_call.id.clone(),
        result: tool_result.clone(),
    });
    
    let tool_result_json = serde_json::json!({
        "toolCallId": tool_call.id,
        "name": tool_call.function.name,
        "result": tool_result,
    })
    .to_string();
    let tool_msg_id = manager.db().add_message(chat_id, "tool", &tool_result_json, None, None)?;
    let tool_message = Message { /* ... */ };
    let _ = event_sender.send(ChatEvent::MessageAdded {
        chat_id,
        message: tool_message,
    });
}
```

### Frontend Fix: Handle duplicate tool calls gracefully

**File**: `frontend/src/lib/chatWs.ts`

Add deduplication logic in `chatToolCallStarted` handler to prevent adding duplicate tool calls:

```typescript
case 'chatToolCallStarted': {
  const { toolCallId, toolName, arguments: args, mcpId } = data as {...};
  const newToolCall = {
    id: toolCallId,
    name: toolName,
    arguments: args,
    status: 'running' as const,
    mcpId,
  };
  pendingToolCalls.update((list) => [...list, newToolCall]);
  
  let tempId = get(streamingMessageId);
  
  if (!tempId) {
    const chatId = get(currentChatId);
    if (!chatId) break;
    
    tempId = `temp-${Date.now()}`;
    const assistantMessage = {
      id: tempId,
      chatId,
      role: 'assistant' as const,
      content: '',
      createdAt: new Date().toISOString(),
      model: get(selectedModel) || '',
      thinkingContent: get(streamingThinkingContent) || undefined,
      toolCalls: [newToolCall],
    };
    messages.update((list) => [...list, assistantMessage as any]);
    streamingMessageId.set(tempId);
  } else {
    messages.update((list) =>
      list.map((m) => {
        if (m.id === tempId) {
          const existingToolCalls = m.toolCalls || [];
          // Prevent duplicate tool calls
          if (existingToolCalls.some((tc) => tc.id === toolCallId)) {
            return m;
          }
          return { ...m, toolCalls: [...existingToolCalls, newToolCall] };
        }
        return m;
      })
    );
  }
  break;
}
```

## Test Case

Create new test case in `frontend/src/tests/e2e/chat-mcp-tools.test.ts`:

```typescript
it('handles tool call retry with correct result ordering', async () => {
  // Setup: configure mock to fail first call, succeed on retry
  let callCount = 0;
  await configureMock((args) => {
    callCount++;
    if (callCount === 1) {
      return 'Error: Access denied - invalid path';
    }
    return 'Success: File created';
  });
  
  // Send message that triggers tool call
  await dispatch({
    type: 'sendMessage',
    payload: { content: 'Create a file', model },
  });
  
  // Wait for completion
  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 15000 });
  
  const allMessages = get(messages);
  
  // Find assistant messages with tool calls
  const assistantMsgsWithToolCalls = allMessages.filter(
    (m) => m.role === 'assistant' && m.toolCalls && m.toolCalls.length > 0
  );
  
  // Should have 2 intermediate messages (one for each tool call attempt)
  expect(assistantMsgsWithToolCalls.length).toBe(2);
  
  // First tool call should have error result
  const firstToolCall = assistantMsgsWithToolCalls[0].toolCalls![0];
  expect(firstToolCall.result).toContain('Access denied');
  
  // Second tool call should have success result
  const secondToolCall = assistantMsgsWithToolCalls[1].toolCalls![0];
  expect(secondToolCall.result).toContain('Success');
  
  // Verify message order
  const firstToolCallIndex = allMessages.findIndex(
    (m) => m.role === 'assistant' && m.toolCalls?.[0]?.result?.includes('Access denied')
  );
  const secondToolCallIndex = allMessages.findIndex(
    (m) => m.role === 'assistant' && m.toolCalls?.[0]?.result?.includes('Success')
  );
  
  expect(firstToolCallIndex).toBeLessThan(secondToolCallIndex);
});
```

## Files to Modify

1. `packages/rhd_chat/src/tools.rs` - Fix event ordering
2. `frontend/src/lib/chatWs.ts` - Add deduplication logic
3. `frontend/src/tests/e2e/chat-mcp-tools.test.ts` - Add test case

## Expected Behavior After Fix

1. User sends message
2. AI decides to call tool → `ToolCallStarted` event creates temp assistant message
3. `MessageAdded` (intermediate) replaces temp message with real message
4. Tool executes → `ToolCallCompleted` updates message with result
5. If tool fails and AI retries:
   - New `ToolCallStarted` creates new temp message
   - New `MessageAdded` replaces it
   - New `ToolCallCompleted` updates with new result
6. Each tool call attempt is displayed as separate assistant message with correct result

## Implementation Steps

1. **Backend** (`packages/rhd_chat/src/tools.rs`):
   - Move `ToolCallStarted` events before `MessageAdded` (intermediate assistant)
   - Keep tool execution and `ToolCallCompleted` in the loop

2. **Frontend** (`frontend/src/lib/chatWs.ts`):
   - Add deduplication check in `chatToolCallStarted` handler
   - Prevent adding tool call if it already exists in the message

3. **Test** (`frontend/src/tests/e2e/chat-mcp-tools.test.ts`):
   - Add test case for tool call retry scenario
   - Verify correct result ordering
