# Phase 5: Add Missing E2E Test for chat-edit-message

## Overview

This phase creates an E2E test for the chat-edit-message state logic. Currently only a UI test exists in `Message.test.ts` that verifies the edit button renders for user messages. This phase adds the state logic test to verify the edit message flow works correctly.

**Scope:**
- Add new test case to `chat-state.test.ts` for edit message flow
- Test steps 5-11 of chat-edit-message.md (steps 1-4 are UI tests)

**Out of scope:**
- UI tests (already exist in Message.test.ts)
- Other test cases

## Files to Modify

### 1. `frontend/src/tests/e2e/chat-state.test.ts`

**Additions:**
Add a new test case for the edit message flow after the existing tests.

#### New Test: `edits message and re-streams response`

**Test Strategy:**
- Create chat and send message (setup steps 1-4)
- Test steps 5-11 (edit and re-stream flow)

**Implementation:**

```typescript
it('edits message and re-streams response', async () => {
  // Covers chat-edit-message.md steps 5-11 (state logic)
  // Steps 1-4 are UI tests (see Message.test.ts)
  
  // Setup: Create chat and send initial message
  await dispatch({ type: 'createChat', payload: { title: 'Edit Test' } });
  await configureMock('First response');
  
  const model = get(availableModels)[0] || 'test_model';
  await dispatch({ type: 'sendMessage', payload: { content: 'Original message', model } });
  
  // Wait for first response to complete
  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 5000 });
  
  let allMessages = get(messages);
  const userMsg = allMessages.find((m) => m.role === 'user');
  expect(userMsg).toBeDefined();
  expect(userMsg!.content).toBe('Original message');
  
  const userMsgId = userMsg!.id;
  const messagesBeforeEdit = allMessages.length;
  
  // Configure mock for edited message response
  await configureMock('Edited response');
  
  // Step 5. System dispatches `editMessage` action with messageId, new content, model
  // Step 6. System updates message content in messages store
  // Step 7. System truncates all messages after edited message
  // Step 8. System sets isStreaming to true
  // Step 9. System sends request to daemon
  await dispatch({
    type: 'editMessage',
    payload: {
      messageId: userMsgId,
      content: 'Edited message',
      model,
    },
  });
  
  // Step 10. Daemon re-processes from edited message
  // Step 11. System receives streaming response (same as send-message flow)
  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 5000 });
  
  // Verify message content updated
  allMessages = get(messages);
  const editedUserMsg = allMessages.find((m) => m.role === 'user');
  expect(editedUserMsg).toBeDefined();
  expect(editedUserMsg!.content).toBe('Edited message');
  
  // Verify subsequent messages removed and new response received
  // After edit: user message + new assistant response
  expect(allMessages.length).toBeLessThanOrEqual(messagesBeforeEdit);
  
  // Verify new response received
  const assistantMsgs = allMessages.filter((m) => m.role === 'assistant');
  expect(assistantMsgs.length).toBeGreaterThan(0);
  
  const finalAssistantMsg = assistantMsgs.find((m) => m.content === 'Edited response');
  expect(finalAssistantMsg).toBeDefined();
}, 15000);
```

## Implementation Notes

1. **Test Isolation**: The test uses `resetAllStores()` in `beforeEach()` to ensure clean state
2. **Setup Phase**: Creates chat and sends initial message to establish conversation state
3. **Edit Flow**: Dispatches `editMessage` action with messageId, new content, and model
4. **Verification**: 
   - Message content is updated
   - Subsequent messages are truncated
   - New response is received from the edited point
5. **Step Coverage**: Steps 5-11 are covered by this test; steps 1-4 (UI edit button click, edit mode, content modification, save) are covered by Message.test.ts

## Dependencies

- This phase depends on: Phase 1 (step comments pattern established)
- This phase must be completed before: Phase 8 (validation)
- Can be done in parallel with Phases 3, 4, 6

## Success Criteria

- New test case added to `chat-state.test.ts`
- Test covers chat-edit-message.md steps 5-11
- Test passes when run with `mise run test-frontend-e2e`
- Step comments clearly map to test case steps
- Sum of step ranges (UI test + E2E test) covers all 11 steps of chat-edit-message.md
