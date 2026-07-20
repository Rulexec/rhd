# Phase 3: Add Missing E2E Test for chat-abort

## Overview

This phase creates an E2E test for the chat-abort state logic. Currently only a UI test exists in `MessageInput.test.ts` that verifies the Abort button renders when streaming. This phase adds the state logic test to verify the abort flow works correctly.

**Scope:**
- Add new test case to `chat-state.test.ts` for abort flow
- Test steps 2-7 of chat-abort.md (step 1 is UI test)

**Out of scope:**
- UI tests (already exist in MessageInput.test.ts)
- Other test cases

## Files to Modify

### 1. `frontend/src/tests/e2e/chat-state.test.ts`

**Additions:**
Add a new test case for the abort flow after the existing tests.

#### New Test: `aborts streaming and updates state`

**Test Strategy:**
- Mock state up to step 3 (streaming in progress)
- Test steps 3-7 (abort flow)

**Implementation:**

```typescript
it('aborts streaming and updates state', async () => {
  // Covers chat-abort.md steps 2-7 (state logic)
  // Step 1 is UI test (see MessageInput.test.ts)
  
  // Setup: Create chat and start streaming
  await dispatch({ type: 'createChat', payload: { title: 'Abort Test' } });
  await configureMock('AI response');
  
  const model = get(availableModels)[0] || 'test_model';
  
  // Start streaming by sending a message
  await dispatch({ type: 'sendMessage', payload: { content: 'Hello', model } });
  
  // Wait for streaming to start
  await waitFor(() => {
    expect(get(isStreaming)).toBe(true);
  }, { timeout: 5000 });
  
  // Step 2. System dispatches `abortChat` action
  // Step 3. System sends abort request to daemon
  await dispatch({ type: 'abortChat' });
  
  // Step 4. Daemon stops processing
  // Step 5. System receives `chatStreamError` or `chatStreamFinished` action
  // The daemon will handle the abort and send back a stream finished/error action
  
  // Step 6. System sets isStreaming to false
  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 5000 });
  
  // Step 7. System clears streamingMessageId
  // Verify streamingMessageId is cleared (imported from chatStores)
  const streamingMsgId = get(streamingMessageId);
  expect(streamingMsgId).toBeNull();
  
  // Verify partial response (if any) remains in message
  const allMessages = get(messages);
  expect(allMessages.length).toBeGreaterThan(0);
  
  // Verify user message exists
  const userMsg = allMessages.find((m) => m.role === 'user');
  expect(userMsg).toBeDefined();
  expect(userMsg!.content).toBe('Hello');
}, 15000);
```

**Required Imports:**
Add `streamingMessageId` to the imports from `chatStores`:

```typescript
import {
  chats,
  currentChatId,
  messages,
  isStreaming,
  availableModels,
  selectedModel,
  streamingMessageId,
  resetAllStores,
} from '../../lib/chatStores';
```

## Implementation Notes

1. **Test Isolation**: The test uses `resetAllStores()` in `beforeEach()` to ensure clean state
2. **Streaming Setup**: The test starts streaming by sending a message, then aborts mid-stream
3. **Timeout Handling**: Uses `waitFor()` with appropriate timeouts for async operations
4. **Partial Response**: The test verifies that any partial response received before abort is preserved
5. **Step Coverage**: Steps 2-7 are covered by this test; step 1 (UI button click) is covered by MessageInput.test.ts

## Dependencies

- This phase depends on: Phase 1 (step comments pattern established)
- This phase must be completed before: Phase 8 (validation)
- Can be done in parallel with Phases 4, 5, 6

## Success Criteria

- New test case added to `chat-state.test.ts`
- Test covers chat-abort.md steps 2-7
- Test passes when run with `mise run test-frontend-e2e`
- Step comments clearly map to test case steps
- Sum of step ranges (UI test + E2E test) covers all 7 steps of chat-abort.md
