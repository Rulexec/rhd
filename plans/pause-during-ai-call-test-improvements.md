# Pause During AI Call Test Improvements

## Overview

This plan addresses 5 improvements to the `pause-during-ai-call.spec.ts` test and related components:

1. **Locator matching improvements** - Replace class names and text matching with testids/data-attributes
2. **Pause button disabled state** - Verify pause button becomes disabled after click
3. **Previous step name display** - Show previous step name in StoryExecutionControls
4. **Queued message ordering** - Ensure queued message appears after loader, not before
5. **Message order after resume** - Fix final message order to match expected behavior

---

## Requirement 1: Locator Matching Improvements

### Current Issues

The test uses several problematic locators:

```typescript
// Class name matching (BAD)
await expect(page.locator('.message.assistant.streaming')).toBeVisible();

// Text matching (FRAGILE)
await expect(page.locator('text=1 message queued')).toBeVisible();
await expect(page.locator('text=Please continue later')).toBeVisible();
await expect(page.locator('text=AI response after resume')).toBeVisible();
```

### Solution

**Add data attributes to components:**

1. **StreamingMessage component** - Add `data-testid="streaming-message"` or `data-streaming-indicator`
2. **Queued messages indicator** - Add `data-queued-messages-count="1"` attribute
3. **User messages** - Add `data-message-content` or similar attribute for content matching
4. **Assistant messages** - Add `data-role="assistant"` and `data-message-id` attributes

**Update testIds.ts:**

```typescript
export const TEST_IDS = {
  MESSAGE_INPUT: 'message-input',
  SEND_BUTTON: 'send-button',
  PAUSE_BUTTON: 'pause-button',
  RESUME_BUTTON: 'resume-button',
  STREAMING_MESSAGE: 'streaming-message',
  QUEUED_INDICATOR: 'queued-indicator',
} as const;
```

**Update components:**

- `StreamingMessage.svelte` - Add `data-testid={TEST_IDS.STREAMING_MESSAGE}`
- `MessageInput.svelte` - Add `data-queued-messages-count={$queuedMessages.length}` to queued indicator
- `Message.svelte` - Add `data-role={message.role}` and `data-message-id={message.id}`

**Update test to use new locators:**

```typescript
// Instead of: page.locator('.message.assistant.streaming')
await expect(page.locator('[data-testid="streaming-message"]')).toBeVisible();

// Instead of: page.locator('text=1 message queued')
await expect(page.locator('[data-queued-messages-count="1"]')).toBeVisible();

// Instead of: page.locator('text=Please continue later')
await expect(page.locator('[data-role="user"][data-message-content*="Please continue later"]')).toBeVisible();
```

---

## Requirement 2: Pause Button Disabled State

### Current Behavior

The test clicks pause but doesn't verify the button becomes disabled:

```typescript
// Step 4. User clicks "click pause" step button
const stepButton1 = page.locator('[data-testid="step-button-1"]');
await stepButton1.click();

// Step 5. System dispatches pauseChat action
// Verify pause button is disabled (UI indicator that pause is being applied)
const pauseButton = page.locator('[data-testid="pause-button"]');
await expect(pauseButton).toBeDisabled();
```

### Issue

The test expects the button to be disabled, but the component logic in `MessageInput.svelte` shows:

```svelte
<button on:click={pause} class="pause-btn" data-testid={TEST_IDS.PAUSE_BUTTON} disabled={isPausePending}>Pause</button>
```

The `isPausePending` state is set to `true` when pause is clicked, but the test needs to verify this happens.

### Solution

The test already has the correct assertion. The component already implements the disabled state via `isPausePending`. No changes needed to the component, but we should verify the test flow is correct.

**Verify in test:**

```typescript
// After clicking pause step button
const pauseButton = page.locator('[data-testid="pause-button"]');
await expect(pauseButton).toBeDisabled();
```

This should work as-is since `isPausePending` is set to `true` in the `pause()` function.

---

## Requirement 3: Previous Step Name Display

### Current Behavior

`StoryExecutionControls.svelte` shows:

```
Step 6 of 10
```

### Desired Behavior

Show previous step name:

```
Step 6 of 10, previous: "click pause"
```

### Solution

**Update `StoryExecutionControls.svelte`:**

```svelte
<script lang="ts">
  // ... existing code ...
  
  let previousStep = $derived(currentStepIndex > 0 ? story.steps[currentStepIndex - 1] : null);
</script>

<div class="step-info">
  <span>Step {currentStepIndex + 1} of {story.steps.length}</span>
  {#if previousStep}
    <span class="previous-step">Previous: {previousStep.name}</span>
  {/if}
  {#if isComplete}
    <span class="complete">✓ All steps completed</span>
  {/if}
</div>
```

**Add CSS:**

```css
.previous-step {
  color: #888;
  font-size: 12px;
  font-style: italic;
}
```

---

## Requirement 4: Queued Message Ordering

### Current Issue

In `MessageList.svelte`:

```svelte
$: allMessages = [...$messages, ...$queuedMessages];
```

This appends queued messages after all regular messages. However, the streaming loader is rendered separately:

```svelte
{#if ($isStreaming || $isPaused) && !$streamingMessageId}
  <StreamingMessage content={$streamingContent} thinkingContent={$streamingThinkingContent} />
{/if}
```

The loader appears AFTER all messages (including queued), but the requirement is that queued messages should appear AFTER the loader.

### Solution

**Reorder rendering in `MessageList.svelte`:**

```svelte
<div class="message-list" bind:this={listElement} on:scroll={handleScroll}>
  <!-- Regular messages -->
  {#each $messages as message, index (message.id)}
    {#if shouldShowModelIndicator(index)}
      <div class="model-indicator">
        <span class="model-indicator-text">Model: {message.model || 'Unknown'}</span>
      </div>
    {/if}
    <Message {message} />
    {#if 'toolCalls' in message && message.toolCalls && message.toolCalls.length > 0}
      <div class="tool-calls-container">
        {#each message.toolCalls as toolCall (toolCall.id)}
          <ToolCallMessage {toolCall} />
        {/each}
      </div>
    {/if}
  {/each}
  
  <!-- Streaming loader (before queued messages) -->
  {#if ($isStreaming || $isPaused) && !$streamingMessageId}
    <StreamingMessage content={$streamingContent} thinkingContent={$streamingThinkingContent} />
  {/if}
  
  <!-- Queued messages (after loader) -->
  {#each $queuedMessages as message, index (message.id)}
    <Message message={message} />
  {/each}
</div>
```

**Update `shouldShowModelIndicator` logic** to account for the split rendering.

---

## Requirement 5: Message Order After Resume

### Current Issue

After final steps, the order is:
1. First user message
2. Queued message
3. "AI response after resume"

### Expected Order

1. First user message
2. "AI response after resume"
3. Queued message
4. New AI chat loader (since we're starting execution of new AI chat request)

### Root Cause

The story's `streamFinished` step adds the AI response to `messages`, but the queued message is still in `queuedMessages`. When the chat resumes, the queued message should be processed and a new stream should start.

### Solution

**Update the story to simulate proper flow:**

In `PauseDuringAiCall.stories.ts`, the `streamFinished` step should:

1. Add AI response to messages
2. Clear queued messages (they've been sent)
3. Start a new stream (set `isStreaming = true`)
4. Show streaming loader

```typescript
{
  name: 'daemon response: streamFinished',
  execute: async ({ state }) => {
    // Add AI response message
    const aiResponseMessage = {
      id: Date.now(),
      chatId: 1,
      role: 'assistant' as const,
      content: 'AI response after resume',
      createdAt: new Date().toISOString(),
      model: 'gpt-4',
    };
    messages.update((list) => [...list, aiResponseMessage]);
    
    // Clear queued messages (they've been processed)
    queuedMessages.set([]);
    
    // Start new stream for the queued message
    isStreaming.set(true);
    streamingMessageId.set(null);
    
    dispatch({ type: 'chatStreamFinished' });
    
    await waitFor(() => {
      if (get(isStreaming) !== true) throw new Error('isStreaming should be true for new request');
    });
    
    return { state: { ...state, step: 9 } };
  },
},
```

**Update test assertions:**

```typescript
// Step 19. System receives chatStreamFinished action
// Verify AI final response is visible
await expect(page.locator('[data-role="assistant"][data-message-content*="AI response after resume"]')).toBeVisible();

// Verify queued message is visible (now being processed)
await expect(page.locator('[data-role="user"][data-message-content*="Please continue later"]')).toBeVisible();

// Verify new streaming indicator is visible (new AI request started)
await expect(page.locator('[data-testid="streaming-message"]')).toBeVisible();
```

---

## Implementation Steps

### Phase 1: Update Components

1. **Update `testIds.ts`** - Add new test IDs for streaming message and queued indicator
2. **Update `StreamingMessage.svelte`** - Add `data-testid` attribute
3. **Update `Message.svelte`** - Add `data-role` and `data-message-id` attributes
4. **Update `MessageInput.svelte`** - Add `data-queued-messages-count` attribute
5. **Update `MessageList.svelte`** - Reorder rendering to place queued messages after loader
6. **Update `StoryExecutionControls.svelte`** - Add previous step name display

### Phase 2: Update Story

7. **Update `PauseDuringAiCall.stories.ts`** - Fix `streamFinished` step to properly simulate new stream start

### Phase 3: Update Test

8. **Update `pause-during-ai-call.spec.ts`** - Replace all class name and text locators with data attributes

### Phase 4: Update Memory

9. **Update `memory/storybook.md`** - Add section on locator matching best practices

---

## Files to Modify

1. `frontend/src/stories/testIds.ts`
2. `frontend/src/components/StreamingMessage.svelte`
3. `frontend/src/components/Message.svelte`
4. `frontend/src/components/MessageInput.svelte`
5. `frontend/src/components/MessageList.svelte`
6. `frontend/src/stories/StoryExecutionControls.svelte`
7. `frontend/src/stories/pause-abort/PauseDuringAiCall.stories.ts`
8. `frontend/tests/storybook/pause-during-ai-call.spec.ts`
9. `memory/storybook.md`

---

## Testing Strategy

After implementation:

1. Run `cd frontend && npm run test-storybook` to verify all tests pass
2. Manually verify the Storybook story shows previous step name
3. Verify message ordering in the story matches expected behavior
4. Verify all locators use data attributes instead of class names or text
