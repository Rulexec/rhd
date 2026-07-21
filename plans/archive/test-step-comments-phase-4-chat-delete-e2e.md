# Phase 4: Add Missing E2E Test for chat-delete

## Overview

This phase creates an E2E test for the chat-delete state logic. Currently only a UI test exists in `ChatList.test.ts` that verifies the delete button renders. This phase adds the state logic test to verify the delete flow works correctly.

**Scope:**
- Add new test case to `chat-state.test.ts` for delete flow
- Test steps 4-8 of chat-delete.md (steps 1-3 are UI tests)

**Out of scope:**
- UI tests (already exist in ChatList.test.ts)
- Other test cases

## Files to Modify

### 1. `frontend/src/tests/e2e/chat-state.test.ts`

**Additions:**
Add a new test case for the delete flow after the existing tests.

#### New Test: `deletes chat and updates state`

**Test Strategy:**
- Create chat first (steps 1-3 setup)
- Test steps 4-8 (delete flow)

**Implementation:**

```typescript
it('deletes chat and updates state', async () => {
  // Covers chat-delete.md steps 4-8 (state logic)
  // Steps 1-3 are UI tests (see ChatList.test.ts)
  
  // Setup: Create a chat first
  await dispatch({ type: 'createChat', payload: { title: 'Chat to Delete' } });
  
  const chatId = get(currentChatId);
  expect(chatId).toBeDefined();
  
  let allChats = get(chats);
  expect(allChats.length).toBe(1);
  
  // Step 4. System dispatches `deleteChat` action with chatId
  // Step 5. System sends request to daemon
  await dispatch({ type: 'deleteChat', payload: { chatId: chatId! } });
  
  // Step 6. Daemon deletes chat
  // Step 7. System removes chat from chats store
  await waitFor(() => {
    allChats = get(chats);
    expect(allChats.length).toBe(0);
  }, { timeout: 5000 });
  
  // Step 8. If deleted chat was current:
  //    - System sets currentChatId to null
  //    - System clears messages store
  expect(get(currentChatId)).toBeNull();
  
  const allMessages = get(messages);
  expect(allMessages.length).toBe(0);
}, 10000);
```

#### New Test: `deletes non-current chat without clearing messages`

**Test Strategy:**
- Create two chats
- Select first chat
- Delete second chat (not current)
- Verify currentChatId remains set and messages are not cleared

**Implementation:**

```typescript
it('deletes non-current chat without clearing messages', async () => {
  // Covers chat-delete.md step 8 condition (state logic)
  // Tests the case where deleted chat is NOT the current chat
  
  // Setup: Create two chats
  await dispatch({ type: 'createChat', payload: { title: 'Chat 1' } });
  const chat1Id = get(currentChatId);
  expect(chat1Id).toBeDefined();
  
  await dispatch({ type: 'createChat', payload: { title: 'Chat 2' } });
  const chat2Id = get(currentChatId);
  expect(chat2Id).toBeDefined();
  expect(chat2Id).not.toBe(chat1Id);
  
  // Select chat 1 as current
  await dispatch({ type: 'selectChat', payload: { chatId: chat1Id! } });
  expect(get(currentChatId)).toBe(chat1Id);
  
  // Add a message to chat 1
  await configureMock('Response');
  const model = get(availableModels)[0] || 'test_model';
  await dispatch({ type: 'sendMessage', payload: { content: 'Hello', model } });
  
  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 5000 });
  
  const messagesBeforeDelete = get(messages);
  expect(messagesBeforeDelete.length).toBeGreaterThan(0);
  
  // Delete chat 2 (not current)
  await dispatch({ type: 'deleteChat', payload: { chatId: chat2Id! } });
  
  await waitFor(() => {
    const allChats = get(chats);
    expect(allChats.length).toBe(1);
    expect(allChats[0].id).toBe(chat1Id);
  }, { timeout: 5000 });
  
  // Verify currentChatId is still set to chat 1
  expect(get(currentChatId)).toBe(chat1Id);
  
  // Verify messages are NOT cleared (because deleted chat was not current)
  const messagesAfterDelete = get(messages);
  expect(messagesAfterDelete.length).toBe(messagesBeforeDelete.length);
}, 15000);
```

## Implementation Notes

1. **Test Isolation**: The test uses `resetAllStores()` in `beforeEach()` to ensure clean state
2. **Two Test Cases**: 
   - First test covers deleting the current chat (clears messages, sets currentChatId to null)
   - Second test covers deleting a non-current chat (preserves messages and currentChatId)
3. **Step Coverage**: Steps 4-8 are covered by these tests; steps 1-3 (UI button click and confirmation dialog) are covered by ChatList.test.ts
4. **Timeout Handling**: Uses `waitFor()` with appropriate timeouts for async operations

## Dependencies

- This phase depends on: Phase 1 (step comments pattern established)
- This phase must be completed before: Phase 8 (validation)
- Can be done in parallel with Phases 3, 5, 6

## Success Criteria

- New test cases added to `chat-state.test.ts`
- Tests cover chat-delete.md steps 4-8
- Tests pass when run with `mise run test-frontend-e2e`
- Step comments clearly map to test case steps
- Sum of step ranges (UI test + E2E tests) covers all 8 steps of chat-delete.md
