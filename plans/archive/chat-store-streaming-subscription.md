# ChatStore Streaming Subscription Implementation

## Overview

Move streaming subscription logic from `ChatView.svelte` component to `ChatStore`. Implement a reactive system using MobX reactions that automatically subscribes to message streaming when a streaming message appears and unsubscribes when it disappears.

## Current State

- `ChatStore.ts` manages chat state and messages
- `ChatView.svelte` currently handles streaming subscriptions directly in the component (lines 46-105, 110-133)
- `Message.svelte` component already has streaming support with `streamContent` prop
- The component shows "Generating response..." when `isStreaming && !displayContent`
- `streamSubscribe` and `onStreamEvents` exist in `chatApiImpl.ts` but are NOT part of the `ChatApi` interface
- Message schema has `isStreaming: boolean` field

## Requirements

1. Add `init()` function to ChatStore that returns a disposer
2. Add getter for first message with `isStreaming: true`
3. Use MobX reaction with `fireImmediately: true` to watch for streaming messages
4. Subscribe to streaming when a streaming message appears
5. Unsubscribe when streaming message disappears
6. Update Message component to stop showing "awaiting request" when content or reasoningContent is not empty
7. Remove existing streaming code from ChatView.svelte
8. Write comprehensive tests for all changes

## Implementation Plan

### Phase 1: ChatStore Changes

#### 1.1 Add Streaming State Fields

Add observable fields to track streaming state:

```typescript
// In ChatStore class
streamingContent: string = '';
streamingReasoningContent: string = '';
streamingToolCalls: StreamToolCallDelta[] = [];
streamingIsFinished: boolean = true;
#cleanupStreamEvents: (() => void) | null = null;
```

#### 1.2 Add streamingMessage Getter

Add a computed getter to find the first message with `isStreaming: true`:

```typescript
get streamingMessage(): Message | null {
  return this.messages.find(m => m.isStreaming) ?? null;
}
```

#### 1.3 Add init() Method

Add an `init()` method that:
- Sets up a MobX reaction watching `streamingMessage`
- When a streaming message appears, call `streamSubscribe` and `onStreamEvents`
- When streaming message disappears, cleanup subscriptions
- Returns a disposer function

```typescript
init(): () => void {
  const disposer = reaction(
    () => this.streamingMessage,
    (message) => {
      if (message) {
        this.#startStreaming(message.chatId);
      } else {
        this.#stopStreaming();
      }
    },
    { fireImmediately: true }
  );

  return () => {
    disposer();
    this.#stopStreaming();
  };
}
```

#### 1.4 Add Private Streaming Methods

Add private methods to handle streaming subscription:

```typescript
async #startStreaming(chatId: number): Promise<void> {
  // Stop any existing streaming first
  this.#stopStreaming();

  // Reset streaming state
  this.streamingContent = '';
  this.streamingReasoningContent = '';
  this.streamingToolCalls = [];
  this.streamingIsFinished = false;

  try {
    // Subscribe to stream and get initial content
    const result = await streamSubscribe(chatId);
    this.streamingContent = result.content;
    this.streamingReasoningContent = result.reasoningContent;
    this.streamingToolCalls = result.toolCalls;
    this.streamingIsFinished = result.isFinished;

    // Register event listeners for stream updates
    this.#cleanupStreamEvents = onStreamEvents(chatId, {
      onStreamChunk: (data) => {
        this.#handleStreamChunk(data);
      },
      onStreamFinished: () => {
        this.#handleStreamFinished();
      }
    });
  } catch (error) {
    console.error('Failed to subscribe to stream:', error);
    this.error = error instanceof Error ? error.message : String(error);
  }
}

#stopStreaming(): void {
  if (this.#cleanupStreamEvents) {
    this.#cleanupStreamEvents();
    this.#cleanupStreamEvents = null;
  }

  // Reset streaming state
  this.streamingContent = '';
  this.streamingReasoningContent = '';
  this.streamingToolCalls = [];
  this.streamingIsFinished = true;
}

#handleStreamChunk(data: StreamChunkData): void {
  switch (data.type) {
    case 'contentDelta':
      if (data.content) {
        this.streamingContent += data.content;
      }
      break;
    case 'reasoningDelta':
      if (data.content) {
        this.streamingReasoningContent += data.content;
      }
      break;
    case 'toolCallDelta':
      if (data.toolCalls) {
        // Merge tool calls by ID
        for (const newCall of data.toolCalls) {
          const existing = this.streamingToolCalls.find(tc => tc.id === newCall.id);
          if (existing) {
            existing.arguments += newCall.arguments;
          } else {
            this.streamingToolCalls.push({ ...newCall });
          }
        }
      }
      break;
  }
}

#handleStreamFinished(): void {
  this.streamingIsFinished = true;
}
```

#### 1.5 Add streamContent Getter

Add a computed getter that returns the current streaming content:

```typescript
get streamContent(): { reasoningContent: string; content: string; toolCalls: StreamToolCallDelta[]; isFinished: boolean } | null {
  if (this.streamingIsFinished && !this.streamingContent && !this.streamingReasoningContent) {
    return null;
  }
  return {
    reasoningContent: this.streamingReasoningContent,
    content: this.streamingContent,
    toolCalls: this.streamingToolCalls,
    isFinished: this.streamingIsFinished
  };
}
```

### Phase 2: ChatView.svelte Changes

#### 2.1 Remove Existing Streaming Code

Remove the following from `ChatView.svelte`:
- Import of `streamSubscribe` and `onStreamEvents` (line 7)
- `StreamSubscription` interface (lines 33-38)
- `streamSubscriptions` state (line 41)
- `unsubscribeStreamEvents` state (line 44)
- `onMount` block that subscribes to stream events (lines 46-101)
- `onDestroy` block that unsubscribes (lines 103-105)
- `ensureStreamSubscription` function (lines 110-126)
- `getStreamContent` function (lines 131-133)
- Auto-scroll effect for streaming (lines 188-197)

#### 2.2 Add init() Call

Add `init()` call in `onMount` and disposer in `onDestroy`:

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getAppStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';
  import Message from './Message.svelte';
  import MessageInput from './MessageInput.svelte';

  const appStore = getAppStore();
  const chatStore = appStore.chat;

  // Bridge MobX observables to Svelte reactivity
  const currentChatGetter = mobxObservable(() => chatStore.currentChat);
  const allMessagesGetter = mobxObservable(() => chatStore.allMessages);
  const chatLoadingGetter = mobxObservable(() => chatStore.loading);
  const chatErrorGetter = mobxObservable(() => chatStore.error);
  const streamContentGetter = mobxObservable(() => chatStore.streamContent);

  let currentChat = $derived(currentChatGetter());
  let allMessages = $derived(allMessagesGetter());
  let chatLoading = $derived(chatLoadingGetter());
  let chatError = $derived(chatErrorGetter());
  let streamContent = $derived(streamContentGetter());

  let disposeChatStore: (() => void) | null = null;

  onMount(() => {
    disposeChatStore = chatStore.init();
  });

  onDestroy(() => {
    if (disposeChatStore) {
      disposeChatStore();
      disposeChatStore = null;
    }
  });

  // ... rest of the component
</script>
```

#### 2.3 Update Message Rendering

Update the message rendering to pass `streamContent` from the store:

```svelte
{#each allMessages as message (message.id)}
  <Message
    {message}
    isQueue={message.isQueue}
    streamContent={message.isStreaming ? streamContent : null}
  />
{/each}
```

### Phase 3: Message Component Changes

#### 3.1 Update "Awaiting Request" Logic

Update the condition for showing "Generating response..." placeholder:

**Current logic** (line 156):
```svelte
{#if isStreaming && !displayContent}
  <span class="streaming-placeholder">Generating response...</span>
```

**New logic**:
```svelte
{#if isStreaming && !displayContent && !displayReasoning}
  <span class="streaming-placeholder">Generating response...</span>
```

This ensures the placeholder is only shown when BOTH content and reasoningContent are empty.

### Phase 4: Testing

#### 4.1 ChatStore Unit Tests

Create or update `frontend/src/stores/ChatStore.test.ts` with the following test cases:

**Test: streamingMessage getter**
- Returns null when no messages have isStreaming: true
- Returns first message when one message has isStreaming: true
- Returns first streaming message when multiple messages have isStreaming: true

**Test: init() method**
- Returns a disposer function
- Sets up reaction that watches streamingMessage
- Calls #startStreaming when streamingMessage appears
- Calls #stopStreaming when streamingMessage disappears
- Disposer cleans up reaction and stops streaming

**Test: streamContent getter**
- Returns null when streamingIsFinished is true and no content
- Returns streaming content object when streaming is active
- Returns correct reasoningContent, content, toolCalls, and isFinished

**Test: #handleStreamChunk**
- Appends content for contentDelta type
- Appends reasoningContent for reasoningDelta type
- Merges tool calls by ID for toolCallDelta type
- Adds new tool calls when ID doesn't exist

**Test: #handleStreamFinished**
- Sets streamingIsFinished to true

**Test: #startStreaming**
- Calls streamSubscribe with correct chatId
- Sets initial streaming state from result
- Calls onStreamEvents with correct handlers
- Handles errors and sets error state

**Test: #stopStreaming**
- Calls cleanup function if it exists
- Resets all streaming state fields

#### 4.2 ChatView Component Tests

Update `frontend/src/lib/components/ChatView.test.ts`:

**Test: init() lifecycle**
- Calls chatStore.init() on mount
- Calls disposer on unmount
- Passes streamContent to Message component for streaming messages

**Test: streaming content display**
- Shows "Generating response..." only when both content and reasoningContent are empty
- Shows content when content is present
- Shows reasoning when reasoningContent is present

#### 4.3 Message Component Tests

Update `frontend/src/lib/components/Message.test.ts`:

**Test: placeholder display logic**
- Shows placeholder when isStreaming is true and both content and reasoningContent are empty
- Does not show placeholder when content is present
- Does not show placeholder when reasoningContent is present
- Shows placeholder when isStreaming is true and streamContent is null

### Phase 5: Post-Implementation

#### 5.1 Update Memory Documentation

After completing this implementation, update `memory/frontend/MEMORY.md` to add a note about testing conventions:

**Add to the "Testing" section:**

```markdown
### Store Testing Requirements

When modifying MobX stores, always write or update unit tests to cover:
- New observable fields and their initial states
- Computed getters and their reactive behavior
- Action methods (both sync and async flows)
- Lifecycle methods (init/dispose patterns)
- Event handlers and their side effects

Tests should verify state transitions, not just final states. Use mocked dependencies to isolate store behavior.
```

## Migration Notes

- The `streamSubscribe` and `onStreamEvents` functions are currently NOT part of the `ChatApi` interface. They are imported directly from `chatApiImpl.ts`. This is intentional per the frontend MEMORY.md: "Streaming functions (`streamSubscribe`/`onStreamEvents`) are intentionally NOT part of the interface."
- The implementation should import these functions directly from `chatApiImpl.ts` in `ChatStore.ts`.

## File Changes Summary

1. `frontend/src/stores/ChatStore.ts`:
   - Add streaming state fields
   - Add `streamingMessage` getter
   - Add `streamContent` getter
   - Add `init()` method
   - Add private streaming methods (`#startStreaming`, `#stopStreaming`, `#handleStreamChunk`, `#handleStreamFinished`)

2. `frontend/src/lib/components/ChatView.svelte`:
   - Remove existing streaming subscription code
   - Add `init()` call in `onMount`
   - Add disposer call in `onDestroy`
   - Add `streamContent` getter from store
   - Update message rendering to pass `streamContent` from store

3. `frontend/src/lib/components/Message.svelte`:
   - Update placeholder condition to check both content and reasoningContent

4. `frontend/src/stores/ChatStore.test.ts`:
   - Add comprehensive tests for streaming functionality

5. `frontend/src/lib/components/ChatView.test.ts`:
   - Update tests for new streaming integration

6. `frontend/src/lib/components/Message.test.ts`:
   - Update tests for placeholder display logic

7. `memory/frontend/MEMORY.md`:
   - Add store testing requirements section
