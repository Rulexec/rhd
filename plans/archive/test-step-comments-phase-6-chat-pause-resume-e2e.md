# Phase 6: Add Missing E2E Test for chat-pause-resume

## Overview

This phase creates an E2E test for the chat-pause-resume state logic. Currently only UI tests exist in `MessageInput.test.ts` that verify the Pause and Resume buttons render correctly. This phase adds the state logic test to verify the pause/resume flow works correctly.

**Scope:**
- Add new test case to `chat-state.test.ts` for pause/resume flow
- Test pause steps 2-7 and resume steps 2-7 (step 1 of each is UI test)

**Out of scope:**
- UI tests (already exist in MessageInput.test.ts)
- Other test cases

## Files to Modify

### 1. `frontend/src/tests/e2e/chat-state.test.ts`

**Additions:**
Add a new test case for the pause/resume flow after the existing tests.

**Required Imports:**
Add `isPaused` to the imports from `chatStores`:

```typescript
import {
  chats,
  currentChatId,
  messages,
  isStreaming,
  isPaused,
  availableModels,
  selectedModel,
  streamingMessageId,
  resetAllStores,
} from '../../lib/chatStores';
```

#### New Test: `pauses and resumes chat during tool loop`

**Test Strategy:**
- Mock state with streaming in progress (setup)
- Test pause steps 2-7
- Test resume steps 2-7

**Implementation:**

```typescript
it('pauses and resumes chat during tool loop', async () => {
  // Covers chat-pause-resume.md pause steps 2-7 and resume steps 2-7 (state logic)
  // Pause step 1 and resume step 1 are UI tests (see MessageInput.test.ts)
  
  // Setup: Create chat and start streaming (simulating tool loop in progress)
  await dispatch({ type: 'createChat', payload: { title: 'Pause/Resume Test' } });
  await configureMock('AI response');
  
  const model = get(availableModels)[0] || 'test_model';
  await dispatch({ type: 'sendMessage', payload: { content: 'Hello', model } });
  
  // Wait for streaming to start
  await waitFor(() => {
    expect(get(isStreaming)).toBe(true);
  }, { timeout: 5000 });
  
  // === PAUSE FLOW ===
  
  // Pause Step 2. System dispatches `pauseChat` action
  // Pause Step 3. System sends pause request to daemon
  await dispatch({ type: 'pauseChat' });
  
  // Pause Step 4. Daemon pauses execution
  // Pause Step 5. System receives `chatPaused` action
  // The daemon will handle the pause and send back a chatPaused action
  
  // Pause Step 6. System sets isPaused to true
  await waitFor(() => {
    expect(get(isPaused)).toBe(true);
  }, { timeout: 5000 });
  
  // Pause Step 7. System sets isStreaming to false
  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 5000 });
  
  // Verify pause state
  expect(get(isPaused)).toBe(true);
  expect(get(isStreaming)).toBe(false);
  
  // === RESUME FLOW ===
  
  // Resume Step 2. System dispatches `resumeChat` action
  // Resume Step 3. System sends resume request to daemon
  await dispatch({ type: 'resumeChat' });
  
  // Resume Step 4. Daemon resumes execution
  // Resume Step 5. System receives `chatResumed` action
  // The daemon will handle the resume and send back a chatResumed action
  
  // Resume Step 6. System sets isPaused to false
  await waitFor(() => {
    expect(get(isPaused)).toBe(false);
  }, { timeout: 5000 });
  
  // Resume Step 7. System sets isStreaming to true
  await waitFor(() => {
    expect(get(isStreaming)).toBe(true);
  }, { timeout: 5000 });
  
  // Verify resume state
  expect(get(isPaused)).toBe(false);
  expect(get(isStreaming)).toBe(true);
  
  // Wait for streaming to complete
  await waitFor(() => {
    expect(get(isStreaming)).toBe(false);
  }, { timeout: 10000 });
  
  // Verify final state
  expect(get(isPaused)).toBe(false);
  expect(get(isStreaming)).toBe(false);
  
  // Verify messages exist
  const allMessages = get(messages);
  expect(allMessages.length).toBeGreaterThan(0);
}, 20000);
```

## Implementation Notes

1. **Test Isolation**: The test uses `resetAllStores()` in `beforeEach()` to ensure clean state
2. **Streaming Setup**: The test starts streaming by sending a message, then pauses mid-stream
3. **Two-Phase Test**: 
   - First phase tests pause flow (steps 2-7)
   - Second phase tests resume flow (steps 2-7)
4. **State Verification**: 
   - After pause: `isPaused` = true, `isStreaming` = false
   - After resume: `isPaused` = false, `isStreaming` = true
5. **Timeout Handling**: Uses `waitFor()` with appropriate timeouts for async operations
6. **Step Coverage**: Pause step 1 and resume step 1 (UI button clicks) are covered by MessageInput.test.ts

## Dependencies

- This phase depends on: Phase 1 (step comments pattern established)
- This phase must be completed before: Phase 8 (validation)
- Can be done in parallel with Phases 3, 4, 5

## Success Criteria

- New test case added to `chat-state.test.ts`
- Test covers chat-pause-resume.md pause steps 2-7 and resume steps 2-7
- Test passes when run with `mise run test-frontend-e2e`
- Step comments clearly map to test case steps
- Sum of step ranges (UI test + E2E test) covers all 7 pause steps and all 7 resume steps of chat-pause-resume.md
