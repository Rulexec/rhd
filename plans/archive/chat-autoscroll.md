# Chat Auto-Scroll Implementation Plan

## Overview

Implement smart auto-scrolling for the chat message list that respects user's scroll position while providing automatic scrolling during streaming.

## Current State

**File**: `frontend/src/components/MessageList.svelte`

Current implementation (lines 10-14):
```typescript
afterUpdate(() => {
  if (listElement) {
    listElement.scrollTop = listElement.scrollHeight;
  }
});
```

**Problem**: Always scrolls to bottom on any update, ignoring user's scroll position.

## Requirements

1. **Auto-scroll when new assistant message appears** - scroll to bottom
2. **Auto-scroll when message height changes** - during streaming (loading → thinking transitions)
3. **Respect user scroll position** - if user scrolled away from bottom, don't auto-scroll
4. **Resume auto-scroll** - if user scrolls back to bottom during streaming, re-enable auto-scroll

## Implementation Strategy

### 1. Track "At Bottom" State

Add a reactive variable to track whether the user is currently at the bottom of the message list:

```typescript
const SCROLL_THRESHOLD = 30; // pixels from bottom
let isAtBottom = true;
```

### 2. Add Scroll Event Listener

Attach a scroll event handler to detect when user manually scrolls:

```typescript
function handleScroll() {
  if (!listElement) return;
  const { scrollTop, scrollHeight, clientHeight } = listElement;
  isAtBottom = scrollHeight - scrollTop - clientHeight < SCROLL_THRESHOLD;
}
```

### 3. Conditional Auto-Scroll in afterUpdate

Modify the afterUpdate hook to only scroll when appropriate:

```typescript
afterUpdate(() => {
  if (listElement && isAtBottom) {
    listElement.scrollTop = listElement.scrollHeight;
  }
});
```

### 4. Reset State on New Streaming

When streaming starts (new message appears), reset `isAtBottom` to `true` to ensure initial auto-scroll:

```typescript
// Watch for streaming state changes
$: if ($isStreaming && !$streamingMessageId) {
  // New streaming started, reset to auto-scroll
  isAtBottom = true;
}
```

## Implementation Details

### File Changes

**`frontend/src/components/MessageList.svelte`**:

1. Add state variables:
   - `isAtBottom: boolean = true`
   - `SCROLL_THRESHOLD: number = 30`

2. Add scroll handler function:
   ```typescript
   function handleScroll() {
     if (!listElement) return;
     const { scrollTop, scrollHeight, clientHeight } = listElement;
     isAtBottom = scrollHeight - scrollTop - clientHeight < SCROLL_THRESHOLD;
   }
   ```

3. Update template to bind scroll event:
   ```svelte
   <div class="message-list" bind:this={listElement} on:scroll={handleScroll}>
   ```

4. Modify afterUpdate:
   ```typescript
   afterUpdate(() => {
     if (listElement && isAtBottom) {
       listElement.scrollTop = listElement.scrollHeight;
     }
   });
   ```

5. Add reactive statement to reset on new streaming:
   ```typescript
   $: if ($isStreaming && !$streamingMessageId) {
     isAtBottom = true;
   }
   ```

## Behavior Matrix

| Scenario | User Position | Action |
|----------|--------------|--------|
| New message arrives | At bottom | Auto-scroll ✓ |
| New message arrives | Scrolled up | No scroll ✗ |
| Streaming content update | At bottom | Auto-scroll ✓ |
| Streaming content update | Scrolled up | No scroll ✗ |
| User scrolls to bottom | Streaming in progress | Resume auto-scroll ✓ |
| User scrolls away | Streaming in progress | Stop auto-scroll ✗ |
| New streaming starts | Any position | Reset to auto-scroll ✓ |

## Testing Scenarios

1. **Initial load**: Messages should scroll to bottom
2. **New message during streaming**: Should auto-scroll if at bottom
3. **Manual scroll up**: Should stop auto-scrolling
4. **Scroll back to bottom**: Should resume auto-scrolling
5. **Message height change (thinking expand)**: Should auto-scroll if at bottom
6. **New streaming session**: Should reset and auto-scroll

## Edge Cases

- **Fast scrolling**: Threshold of 30px provides buffer for imprecise scrolling
- **Programmatic scroll**: Only user scroll events trigger the handler
- **Component unmount**: Scroll listener automatically cleaned up by Svelte
- **Empty message list**: No scroll needed, `isAtBottom` remains true

## References

- Similar pattern used in `Message.svelte` for thinking content auto-scroll (lines 16, 41-51)
- Uses same `SCROLL_THRESHOLD` approach for consistency
