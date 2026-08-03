# Plan: Pause During AI Call Story Implementation

## Overview
Create a Storybook story with step-by-step controls to test the pause button functionality during an AI call, following the test case defined in `tests/cases/pause-abort/pause-during-ai-call.md`.

## Key Design Principle
**Split user actions from mocked daemon responses** - Each step should either:
- Perform a user action (click button, type text)
- OR dispatch a mocked WebSocket response (chatPaused, messageQueued, chatResumed)

This allows manual verification of each state transition.

## Files to Create/Modify

### 1. `frontend/src/stories/testIds.ts` (Modify)
Add test IDs for pause and resume buttons:

```typescript
export const TEST_IDS = {
  MESSAGE_INPUT: 'message-input',
  SEND_BUTTON: 'send-button',
  PAUSE_BUTTON: 'pause-button',
  RESUME_BUTTON: 'resume-button',
} as const;
```

### 2. `frontend/src/components/MessageInput.svelte` (Modify)
Update pause and resume buttons to use test IDs:

```svelte
{#if showPauseButton}
  <button on:click={pause} class="pause-btn" data-testid={TEST_IDS.PAUSE_BUTTON}>Pause</button>
{/if}

{#if showResumeButton}
  <button on:click={resume} class="resume-btn" data-testid={TEST_IDS.RESUME_BUTTON}>Resume</button>
{/if}
```

### 3. `frontend/src/stories/testUtils.ts` (Modify)
Add `waitFor` utility function that uses assertion-based polling:

```typescript
/**
 * Wait for assertions to pass within a timeout.
 * Repeatedly calls the assertion function until it passes or times out.
 * 
 * @param assertions - Function containing expect() assertions
 * @param options - timeout (ms) and interval (ms) between retries
 * @throws The last error if timeout is reached
 * 
 * @example
 * await waitFor(() => {
 *   expect(get(isPaused)).toBe(true);
 *   expect(get(isStreaming)).toBe(false);
 * });
 */
export async function waitFor(
  assertions: () => void,
  options: { timeout?: number; interval?: number } = {}
): Promise<void> {
  const { timeout = 5000, interval = 50 } = options;
  const startTime = Date.now();
  let lastError: Error | null = null;
  
  while (Date.now() - startTime < timeout) {
    try {
      assertions();
      return; // Assertions passed
    } catch (err) {
      lastError = err instanceof Error ? err : new Error(String(err));
      await sleep(interval);
    }
  }
  
  // Timeout reached, rethrow the last error
  throw lastError ?? new Error(`waitFor timed out after ${timeout}ms`);
}
```

### 4. `frontend/src/stories/pause-abort/ChatViewWithControls.svelte` (Create)
Wrapper component that combines:
- `ChatView` component (the actual UI)
- `StoryExecutionControls` component (step-by-step controls)

```svelte
<script lang="ts">
  import ChatView from '@/components/ChatView.svelte';
  import StoryExecutionControls from '@/stories/StoryExecutionControls.svelte';
  import type { StoryControlDefinition } from '@/stories/StoryExecutionControls.svelte';
  
  export let story: StoryControlDefinition<any>;
</script>

<div class="story-container">
  <StoryExecutionControls {story} />
  <div class="chat-view-wrapper">
    <ChatView />
  </div>
</div>

<style>
  .story-container {
    display: flex;
    flex-direction: column;
    height: 100vh;
  }
  
  .chat-view-wrapper {
    flex: 1;
    overflow: hidden;
  }
</style>
```

### 5. `frontend/src/stories/pause-abort/PauseDuringAiCall.stories.ts` (Create)
Story definition with steps matching the test case.

## Story Steps Design

### Phase 1: Setup (Preconditions)
**Step 1: "setup"**
- Reset all stores
- Create mock chat and set as current
- Set `isStreaming = true`, `isPaused = false`
- Set `availableModels` and `selectedModel`
- Mock `pauseChat`, `resumeChat`, `queueMessage` WebSocket handlers

### Phase 2: Pause Phase
**Step 2: "click pause"**
- Find pause button using `TEST_IDS.PAUSE_BUTTON`
- Click pause button
- This dispatches `pauseChat` action

**Step 3: "daemon response: chatPaused"**
- Dispatch `chatPaused` action (simulates daemon response)
- Use `waitFor` to verify: `isPaused = true`, `isStreaming = false`

### Phase 3: Message Queue Phase
**Step 4: "type message"**
- Find message input textarea using `TEST_IDS.MESSAGE_INPUT`
- Type "Please continue later"

**Step 5: "click send (queue)"**
- Find send button using `TEST_IDS.SEND_BUTTON` (should show "Queue" text when paused)
- Click send button
- This dispatches `queueMessage` action

**Step 6: "daemon response: messageQueued"**
- Dispatch `messageQueued` action (simulates daemon response)
- Use `waitFor` to verify: `queuedMessages.length = 1`

### Phase 4: Resume Phase
**Step 7: "click resume"**
- Find resume button using `TEST_IDS.RESUME_BUTTON`
- Click resume button
- This dispatches `resumeChat` action

**Step 8: "daemon response: chatResumed"**
- Dispatch `chatResumed` action (simulates daemon response)
- Use `waitFor` to verify: `isPaused = false`, `isStreaming = true`, `queuedMessages.length = 0`

**Step 9: "daemon response: streamFinished"**
- Dispatch `chatStreamFinished` action
- Use `waitFor` to verify: `isStreaming = false`

### Phase 5: Verification
**Step 10: "verify final state"**
- Check that all messages are displayed
- Check that queued message was processed
- Check that `isPaused = false`, `isStreaming = false`

## Implementation Details

### Mock WebSocket Handlers
```typescript
setMockWsHandler('pauseChat', async (request) => {
  return { type: 'response', id: request.id as string, success: true, data: {} };
});

setMockWsHandler('resumeChat', async (request) => {
  return { type: 'response', id: request.id as string, success: true, data: {} };
});

setMockWsHandler('queueMessage', async (request) => {
  return { type: 'response', id: request.id as string, success: true, data: {} };
});
```

### State Verification with waitFor
Use `waitFor` with assertions to wait for specific conditions:

```typescript
import { waitFor } from '@/stories/testUtils';
import { isPaused, isStreaming, queuedMessages } from '@/lib/chatStores';
import { get } from 'svelte/store';
import { expect } from 'vitest';

// Wait for pause to take effect
await waitFor(() => {
  expect(get(isPaused)).toBe(true);
  expect(get(isStreaming)).toBe(false);
});

// Wait for message to be queued
await waitFor(() => {
  expect(get(queuedMessages)).toHaveLength(1);
});

// Wait for resume to take effect
await waitFor(() => {
  expect(get(isPaused)).toBe(false);
  expect(get(isStreaming)).toBe(true);
  expect(get(queuedMessages)).toHaveLength(0);
});
```

### Test IDs
Use test IDs from `testIds.ts`:
- `TEST_IDS.MESSAGE_INPUT` - message textarea
- `TEST_IDS.SEND_BUTTON` - send/queue button
- `TEST_IDS.PAUSE_BUTTON` - pause button
- `TEST_IDS.RESUME_BUTTON` - resume button

## Execution Order
1. Add test IDs to `testIds.ts`
2. Update `MessageInput.svelte` to use test IDs
3. Add `waitFor` utility to `testUtils.ts`
4. Create `ChatViewWithControls.svelte`
5. Create `PauseDuringAiCall.stories.ts` with all steps
6. Verify story loads in Storybook
7. Hand off to user for manual step-by-step verification

## Notes
- The story will be implemented first, then given to the user for manual testing
- After user verification, we'll write the actual Playwright test
- Each step should be atomic and testable independently
- Use `waitFor` with assertions instead of `sleep` for reliable state verification
