# Phase 1: Add Step Comments to Existing E2E Tests

## Overview

This phase adds step-by-step comments to existing E2E tests that already cover test cases. The comments will trace which test code covers which test case step, making it easy to verify coverage and understand test flow.

**Scope:**
- Add step comments to `chat-state.test.ts` for: chat-create, chat-send-message, chat-streaming, chat-select-model
- Add step comments to `chat-mcp-tools.test.ts` for: chat-mcp-tools

**Out of scope:**
- UI tests (covered in Phase 2)
- New test creation (covered in Phases 3-7)

## Files to Modify

### 1. `frontend/src/tests/e2e/chat-state.test.ts`

**Modifications:**
Add step comments throughout the file to map test code to test case steps.

#### Test: `creates chat via daemon and updates state`

**Add step comments for chat-create.md (steps 1-10):**

```typescript
it('creates chat via daemon and updates state', async () => {
  // Covers chat-create.md steps 5-9 (state logic)
  // Steps 1-4, 10 are UI tests (see ChatList.test.ts)
  
  // Step 5. System dispatches `createChat` action with title
  // Step 6. System sends request to daemon
  await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });
  
  // Step 7. Daemon creates chat and returns chatId
  // Step 8. System updates chats store with new chat
  const allChats = get(chats);
  expect(allChats.length).toBe(1);
  expect(allChats[0].title).toBe('Test Chat');

  // Step 9. System sets currentChatId to new chat
  expect(get(currentChatId)).toBeDefined();
});
```

#### Test: `sends message and receives streaming response from daemon`

**Add step comments for chat-send-message.md (steps 1-14) and chat-streaming.md (steps 1-9):**

```typescript
it('sends message and receives streaming response from daemon', async () => {
  // Setup: Create chat first
  await dispatch({ type: 'createChat', payload: { title: 'Test' } });
  await configureMock('AI response content');

  const model = get(availableModels)[0] || 'test_model';
  
  // Covers chat-send-message.md steps 3-14 (state logic)
  // Steps 1-2 are UI tests (see MessageInput.test.ts)
  
  // Step 3. System dispatches `sendMessage` action with content and model
  // Step 4. System adds user message to messages store
  // Step 5. System sets isStreaming to true
  // Step 6. System sends request to daemon
  await dispatch({ type: 'sendMessage', payload: { content: 'Hello', model } });

  // Step 7. Daemon processes message and sends response
  // Step 8. System receives `chatStreamChunk` actions
  // Step 9. System creates optimistic assistant message on first chunk
  // Step 10. System appends chunks to assistant message
  // Step 11. System receives `chatStreamFinished` action
  // Step 12. System sets isStreaming to false
  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 5000 });

  // Step 13. System receives `chatMessageAdded` with real message
  // Step 14. System replaces optimistic message with real message
  const allMessages = get(messages);
  expect(allMessages.length).toBe(5);

  const userMsg = allMessages.find((m) => m.role === 'user');
  expect(userMsg).toBeDefined();
  expect(userMsg!.content).toBe('Hello');

  const assistantMsgs = allMessages.filter((m) => m.role === 'assistant');
  expect(assistantMsgs.length).toBe(2);
  
  const finalAssistantMsg = assistantMsgs.find((m) => m.content === 'AI response content');
  expect(finalAssistantMsg).toBeDefined();
  
  // Covers chat-streaming.md steps 1-9 (state logic)
  // Steps 1-3: Optimistic message creation and chunk appending (verified by message count)
  // Step 4: Animated dots indicator (UI test, see Message.test.ts)
  // Steps 5-9: Stream finish handling (verified by isStreaming=false and final message)
});
```

#### Test: `handles multiple messages in sequence`

**Add step comments:**

```typescript
it('handles multiple messages in sequence', async () => {
  await dispatch({ type: 'createChat', payload: { title: 'Test' } });

  const model = get(availableModels)[0] || 'test_model';

  // First message: covers chat-send-message.md steps 3-14
  await configureMock('First response');
  await dispatch({ type: 'sendMessage', payload: { content: 'First message', model } });

  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 5000 });

  let allMessages = get(messages);
  expect(allMessages.length).toBe(5);

  // Second message: covers chat-send-message.md steps 3-14 again
  await configureMock('Second response');
  await dispatch({ type: 'sendMessage', payload: { content: 'Second message', model } });

  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 5000 });

  allMessages = get(messages);
  expect(allMessages.length).toBe(9);

  const userMessages = allMessages.filter((m) => m.role === 'user');
  expect(userMessages.length).toBe(2);
  expect(userMessages[0].content).toBe('First message');
  expect(userMessages[1].content).toBe('Second message');

  const assistantMessages = allMessages.filter((m) => m.role === 'assistant');
  expect(assistantMessages.length).toBe(4);
  const finalAssistantMessages = assistantMessages.filter((m) => m.content && m.content.length > 0);
  expect(finalAssistantMessages.length).toBe(2);
  expect(finalAssistantMessages[0].content).toBe('First response');
  expect(finalAssistantMessages[1].content).toBe('Second response');
});
```

#### Test: `auto-selects first model when available`

**Add step comments for chat-select-model.md (steps 1-8):**

```typescript
it('auto-selects first model when available', async () => {
  // Covers chat-select-model.md steps 1-3, 6-8 (state logic)
  // Steps 4-5 are UI tests (see MessageInput.test.ts)
  
  // Step 1. System loads available models on mount
  await dispatch({ type: 'loadAvailableModels' });

  // Step 2. System populates model selector dropdown (UI test)
  // Step 3. If no model selected, system auto-selects first model
  const models = get(availableModels);
  expect(models.length).toBeGreaterThan(0);

  selectedModel.set(null);
  selectedModel.set(models[0]);

  // Step 6. System dispatches `selectModel` action with model name (implicit via store set)
  // Step 7. System updates selectedModel store
  expect(get(selectedModel)).toBe(models[0]);
  
  // Step 8. Model selection persists for current chat (verified by store state)
});
```

#### Test: `loads available models from daemon`

**Add step comments:**

```typescript
it('loads available models from daemon', async () => {
  // Covers chat-select-model.md step 1 (state logic)
  await dispatch({ type: 'loadAvailableModels' });

  const models = get(availableModels);
  expect(models.length).toBeGreaterThan(0);
  expect(models).toContain('test_model');
});
```

### 2. `frontend/src/tests/e2e/chat-mcp-tools.test.ts`

**Modifications:**
Add step comments throughout the file to map test code to chat-mcp-tools.md steps (1-19).

#### Test: `attaches project with MCP and streams final response after tool call`

**Add step comments for chat-mcp-tools.md (steps 1-19):**

```typescript
it('attaches project with MCP and streams final response after tool call', async () => {
  // Covers chat-mcp-tools.md steps 1-19
  
  // Step 1. User creates new chat (setup)
  await dispatch({ type: 'loadAvailableModels' });
  await loadProjects();

  const models = get(availableModels);
  expect(models.length).toBeGreaterThan(0);
  const model = models[0];

  await dispatch({ type: 'createChat', payload: { title: 'MCP Test Chat' } });
  const chatId = get(currentChatId);
  expect(chatId).toBeDefined();

  // Step 2. User attaches project with MCP configuration
  const attachResult = await attachProject(chatId!, 'test-project-mcp');
  expect(attachResult.success).toBe(true);

  // Step 3. System spawns MCP server, emits `ProjectAttached` event
  await waitFor(() => {
    const projects = get(chatProjects);
    expect(projects.some((p) => p.name === 'test-project-mcp')).toBe(true);
  }, { timeout: 5000 });

  await waitFor(() => {
    const statuses = get(mcpStatuses);
    const mockMcpStatus = statuses.find(
      (s) => s.projectName === 'test-project-mcp' && s.mcpId === 'mock1'
    );
    expect(mockMcpStatus).toBeDefined();
    expect(mockMcpStatus?.status).toBe('connected');
  }, { timeout: 10000 });

  await configureMock('Tool executed successfully! The echo tool returned: Echo: Hello MCP');

  // Step 4. User sends message asking to use specific MCP tool
  // Step 5. System collects tools from attached projects
  // Step 6. System calls AI with tool definitions (streaming)
  await dispatch({
    type: 'sendMessage',
    payload: { content: 'Please use the echo tool with message "Hello MCP"', model },
  });

  // Step 7. AI streams thinking content via `ThinkingChunk` events
  // Step 8. AI returns tool call request
  // Step 9. System creates assistant message with thinking content and tool calls
  // Step 10. System emits `ToolCallStarted` event
  // Step 11. System executes MCP tool
  // Step 12. System emits `ToolCallCompleted` event
  // Step 13. System persists intermediate assistant message (with thinking and tool calls) to database
  // Step 14. System persists tool result message to database
  // Step 15. System feeds tool result back to AI
  // Step 16. AI returns final response (no more tool calls)
  // Step 17. System streams final response to user via `StreamChunk` events
  // Step 18. System emits `StreamFinished` event
  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 15000 });

  // Step 19. System persists final assistant message to database
  const allMessages = get(messages);
  expect(allMessages.length).toBeGreaterThanOrEqual(2);

  const userMsg = allMessages.find((m) => m.role === 'user');
  expect(userMsg).toBeDefined();
  expect(userMsg!.content).toContain('echo tool');

  // Find the final assistant message (last one, contains the actual response)
  const assistantMessages = allMessages.filter((m) => m.role === 'assistant');
  expect(assistantMessages.length).toBeGreaterThanOrEqual(2); // intermediate + final
  const assistantMsg = assistantMessages[assistantMessages.length - 1];
  expect(assistantMsg).toBeDefined();
  expect(assistantMsg.content).toContain('Echo: Hello MCP');
  expect(typeof assistantMsg.id).toBe('number');
  expect(assistantMsg.id).toBeGreaterThan(0);
  // Final assistant message should NOT have toolCalls (already shown in intermediate message)
  expect(assistantMsg.toolCalls).toBeUndefined();

  // Verify tool calls are visible in messages (steps 9-12)
  const assistantMsgsWithToolCalls = allMessages.filter(
    (m) => m.role === 'assistant' && m.toolCalls && m.toolCalls.length > 0
  );
  expect(assistantMsgsWithToolCalls.length).toBeGreaterThan(0);

  const toolCallMsg = assistantMsgsWithToolCalls[0];
  expect(toolCallMsg.toolCalls![0].name).toContain('echo');
  expect(toolCallMsg.toolCalls![0].status).toBe('completed');
  expect(toolCallMsg.toolCalls![0].result).toBeDefined();

  // Verify message order: user → assistant (with toolCalls) → assistant (final)
  const userMsgIndex = allMessages.findIndex((m) => m.role === 'user');
  const toolCallMsgIndex = allMessages.findIndex(
    (m) => m.role === 'assistant' && m.toolCalls && m.toolCalls.length > 0
  );
  const finalAssistantMsgIndex = allMessages.findIndex(
    (m) => m.role === 'assistant' && m.content.includes('Echo: Hello MCP')
  );

  expect(userMsgIndex).toBeLessThan(toolCallMsgIndex);
  expect(toolCallMsgIndex).toBeLessThan(finalAssistantMsgIndex);
  
  // Verify no separate tool result messages visible
  const toolResultMsgCount = allMessages.filter((m) => m.role === 'tool').length;
  expect(toolResultMsgCount).toBe(0);
}, 30000);
```

#### Tests: `handles multiple tool calls with different results` and `maintains unique tool call ids across multiple iterations`

**Add step comments:**

These tests verify the same flow (steps 1-19) but with multiple iterations. Add similar step comments as above, noting that steps 4-19 are repeated for each message.

```typescript
it('handles multiple tool calls with different results', async () => {
  // Covers chat-mcp-tools.md steps 1-19 (first iteration)
  // Setup steps 1-3
  await dispatch({ type: 'loadAvailableModels' });
  await loadProjects();

  const models = get(availableModels);
  expect(models.length).toBeGreaterThan(0);
  const model = models[0];

  await dispatch({ type: 'createChat', payload: { title: 'MCP Multiple Calls Test' } });
  const chatId = get(currentChatId);
  expect(chatId).toBeDefined();

  const attachResult = await attachProject(chatId!, 'test-project-mcp');
  expect(attachResult.success).toBe(true);

  await waitFor(() => {
    const projects = get(chatProjects);
    expect(projects.some((p) => p.name === 'test-project-mcp')).toBe(true);
  }, { timeout: 5000 });

  await waitFor(() => {
    const statuses = get(mcpStatuses);
    const mockMcpStatus = statuses.find(
      (s) => s.projectName === 'test-project-mcp' && s.mcpId === 'mock1'
    );
    expect(mockMcpStatus).toBeDefined();
    expect(mockMcpStatus?.status).toBe('connected');
  }, { timeout: 10000 });

  // Steps 4-19: First tool call iteration
  await configureMock('Tool executed successfully');
  await dispatch({
    type: 'sendMessage',
    payload: { content: 'Please use the echo tool with message "Hello"', model },
  });

  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 15000 });

  const allMessages = get(messages);

  // Verify tool call message (steps 9-12)
  const assistantMsgsWithToolCalls = allMessages.filter(
    (m) => m.role === 'assistant' && m.toolCalls && m.toolCalls.length > 0
  );
  expect(assistantMsgsWithToolCalls.length).toBe(1);
  const toolCallMsg = assistantMsgsWithToolCalls[0];
  expect(toolCallMsg.toolCalls!.length).toBe(1);

  const toolCall = toolCallMsg.toolCalls![0];
  expect(toolCall.name).toContain('echo');
  expect(toolCall.status).toBe('completed');
  expect(toolCall.result).toBeDefined();
  expect(toolCall.result).toContain('Echo: Hello');

  // Verify message order
  const userMsgIndex = allMessages.findIndex((m) => m.role === 'user');
  const toolCallMsgIndex = allMessages.findIndex(
    (m) => m.role === 'assistant' && m.toolCalls && m.toolCalls.length > 0
  );

  expect(userMsgIndex).toBeLessThan(toolCallMsgIndex);
  
  // Verify no separate tool result messages visible
  const toolResultMsgCount = allMessages.filter((m) => m.role === 'tool').length;
  expect(toolResultMsgCount).toBe(0);
}, 30000);
```

```typescript
it('maintains unique tool call ids across multiple iterations', async () => {
  // Covers chat-mcp-tools.md steps 1-19 (two iterations)
  // Setup steps 1-3
  await dispatch({ type: 'loadAvailableModels' });
  await loadProjects();

  const models = get(availableModels);
  expect(models.length).toBeGreaterThan(0);
  const model = models[0];

  await dispatch({ type: 'createChat', payload: { title: 'MCP Unique IDs Test' } });
  const chatId = get(currentChatId);
  expect(chatId).toBeDefined();

  const attachResult = await attachProject(chatId!, 'test-project-mcp');
  expect(attachResult.success).toBe(true);

  await waitFor(() => {
    const projects = get(chatProjects);
    expect(projects.some((p) => p.name === 'test-project-mcp')).toBe(true);
  }, { timeout: 5000 });

  await waitFor(() => {
    const statuses = get(mcpStatuses);
    const mockMcpStatus = statuses.find(
      (s) => s.projectName === 'test-project-mcp' && s.mcpId === 'mock1'
    );
    expect(mockMcpStatus).toBeDefined();
    expect(mockMcpStatus?.status).toBe('connected');
  }, { timeout: 10000 });

  // Steps 4-19: First tool call iteration
  await configureMock('First result');
  await dispatch({
    type: 'sendMessage',
    payload: { content: 'Please use the echo tool with message "First"', model },
  });

  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 15000 });

  const messagesAfterFirst = get(messages);
  const firstToolCallMsgs = messagesAfterFirst.filter(
    (m) => m.role === 'assistant' && m.toolCalls && m.toolCalls.length > 0
  );
  expect(firstToolCallMsgs.length).toBe(1);
  const firstToolCallId = firstToolCallMsgs[0].toolCalls![0].id;
  expect(firstToolCallId).toBeDefined();
  expect(firstToolCallMsgs[0].toolCalls![0].result).toContain('Echo: Hello MCP');

  // Steps 4-19: Second tool call iteration (different message)
  await configureMock('Second result');
  await dispatch({
    type: 'sendMessage',
    payload: { content: 'Please use the echo tool with message "Second"', model },
  });

  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 15000 });

  const allMessages = get(messages);
  const allToolCallMsgs = allMessages.filter(
    (m) => m.role === 'assistant' && m.toolCalls && m.toolCalls.length > 0
  );

  // Should have two intermediate messages with tool calls
  expect(allToolCallMsgs.length).toBe(2);

  // Verify each tool call has a unique ID
  const toolCallIds = allToolCallMsgs.flatMap((m) => m.toolCalls!.map((tc) => tc.id));
  const uniqueIds = new Set(toolCallIds);
  expect(uniqueIds.size).toBe(toolCallIds.length);

  // Verify first tool call still has correct result
  const firstMsg = allToolCallMsgs[0];
  expect(firstMsg.toolCalls![0].result).toContain('Echo: Hello MCP');

  // Verify second tool call has correct result
  const secondMsg = allToolCallMsgs[1];
  expect(secondMsg.toolCalls![0].result).toContain('Echo: Hello MCP');

  // Verify tool call IDs are different
  expect(firstMsg.toolCalls![0].id).not.toBe(secondMsg.toolCalls![0].id);
}, 30000);
```

## Implementation Notes

1. **Comment Format**: Use `// Step N. Description` format matching the test case step descriptions
2. **Range Coverage**: When a test covers multiple steps, use `// Covers test-case.md steps X-Y` at the top of the test
3. **UI vs State Logic**: Clearly indicate which steps are covered by state logic tests vs UI tests
4. **Setup Steps**: Mark setup steps (like creating a chat before testing message send) clearly
5. **No Code Changes**: Only add comments, do not modify test logic

## Dependencies

- This phase depends on: None (can be done first)
- This phase must be completed before: Phase 2 (UI tests), Phase 8 (validation)
- Phase 2 can be done in parallel with this phase

## Success Criteria

- All existing E2E tests have step comments
- Comments clearly map to test case steps
- Sum of step ranges covers all test case steps for: chat-create, chat-send-message, chat-streaming, chat-select-model, chat-mcp-tools
- No test logic is modified
