# Phase 5: Frontend Streaming Subscription & Rendering

## Overview

Update the frontend to detect streaming messages, subscribe to streams via the new API, and reactively render streaming content as it arrives.

## Scope

- Add `isFinished` and `isStreaming` fields to `MessageSchema`
- Add `streamSubscribe` method to the WebSocket client
- Handle `streamChunk` and `streamFinished` events
- Detect streaming messages in `ChatView` and subscribe to their streams
- Update `Message` component to render streaming content reactively
- Reuse existing streaming UI (animated dots, auto-scroll)

## Dependencies

- Phase 2 (stream manager must exist in server).
- Phase 3 (stream API methods must be defined).

---

## Files to Modify

### 1. `frontend/src/lib/api/schemas.ts`

**Add streaming fields to `MessageSchema`:**

```typescript
export const MessageSchema = z.object({
  id: z.number(),
  chatId: z.number(),
  role: z.string(),
  content: z.string(),
  createdAt: z.string(),
  reasoningContent: z.string().nullable().optional(),
  tags: z.array(z.string()).default([]),
  isFinished: z.boolean().default(true),
  isStreaming: z.boolean().default(false),
});

export type Message = z.infer<typeof MessageSchema>;
```

---

### 2. `frontend/src/lib/api/client.ts`

**Add `streamSubscribe` method:**

```typescript
export class ChatClient {
  // ... existing methods ...

  /**
   * Subscribe to a stream and get current accumulated content.
   */
  async streamSubscribe(chatId: number): Promise<StreamSubscribeResult> {
    return this.sendRequest('streamSubscribe', { chatId });
  }
}

export interface StreamSubscribeResult {
  reasoningContent: string;
  content: string;
  toolCalls: StreamToolCallDelta[];
  isFinished: boolean;
}

export interface StreamToolCallDelta {
  id: string;
  name: string;
  arguments: string;
}
```

**Add event handlers for stream events:**

```typescript
export class ChatClient {
  // ... existing event handlers ...

  /**
   * Register a handler for streamChunk events.
   */
  onStreamChunk(handler: (data: StreamChunkData) => void): () => void {
    return this.onEvent('streamChunk', handler);
  }

  /**
   * Register a handler for streamFinished events.
   */
  onStreamFinished(handler: (data: StreamFinishedData) => void): () => void {
    return this.onEvent('streamFinished', handler);
  }
}

export interface StreamChunkData {
  chatId: number;
  type: 'reasoningDelta' | 'contentDelta' | 'toolCallDelta';
  content?: string;
  toolCalls?: StreamToolCallDelta[];
}

export interface StreamFinishedData {
  chatId: number;
}
```

---

### 3. `frontend/src/lib/components/ChatView.svelte`

**Add streaming state management:**

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import type { Message, StreamChunkData, StreamFinishedData } from '$lib/api/schemas';

  // Track active stream subscriptions
  const streamSubscriptions = new Map<number, {
    reasoningContent: string;
    content: string;
    toolCalls: any[];
    isFinished: boolean;
  }>();

  // Subscribe to stream events
  let unsubscribeStreamChunk: (() => void) | null = null;
  let unsubscribeStreamFinished: (() => void) | null = null;

  onMount(() => {
    // Subscribe to streamChunk events
    unsubscribeStreamChunk = client.onStreamChunk((data: StreamChunkData) => {
      const chatId = data.chatId;
      const subscription = streamSubscriptions.get(chatId);
      
      if (subscription) {
        if (data.type === 'reasoningDelta' && data.content) {
          subscription.reasoningContent += data.content;
        } else if (data.type === 'contentDelta' && data.content) {
          subscription.content += data.content;
        } else if (data.type === 'toolCallDelta' && data.toolCalls) {
          // Merge tool calls by ID
          for (const tc of data.toolCalls) {
            const existing = subscription.toolCalls.find(t => t.id === tc.id);
            if (existing) {
              existing.arguments += tc.arguments;
            } else {
              subscription.toolCalls.push({ ...tc });
            }
          }
        }
        
        // Trigger reactivity
        streamSubscriptions = new Map(streamSubscriptions);
      }
    });

    // Subscribe to streamFinished events
    unsubscribeStreamFinished = client.onStreamFinished((data: StreamFinishedData) => {
      const chatId = data.chatId;
      const subscription = streamSubscriptions.get(chatId);
      
      if (subscription) {
        subscription.isFinished = true;
        // Trigger reactivity
        streamSubscriptions = new Map(streamSubscriptions);
        
        // Clean up subscription after a delay (allow final render)
        setTimeout(() => {
          streamSubscriptions.delete(chatId);
          streamSubscriptions = new Map(streamSubscriptions);
        }, 1000);
      }
    });
  });

  onDestroy(() => {
    unsubscribeStreamChunk?.();
    unsubscribeStreamFinished?.();
  });

  /**
   * Check if a message is streaming and subscribe to its stream if needed.
   */
  async function ensureStreamSubscription(message: Message) {
    if (message.isStreaming && !streamSubscriptions.has(message.chatId)) {
      try {
        const result = await client.streamSubscribe(message.chatId);
        streamSubscriptions.set(message.chatId, {
          reasoningContent: result.reasoningContent,
          content: result.content,
          toolCalls: result.toolCalls || [],
          isFinished: result.isFinished,
        });
        // Trigger reactivity
        streamSubscriptions = new Map(streamSubscriptions);
      } catch (error) {
        console.error('Failed to subscribe to stream:', error);
      }
    }
  }

  /**
   * Get the streaming content for a message, if any.
   */
  function getStreamContent(message: Message): {
    reasoningContent: string;
    content: string;
    toolCalls: any[];
    isFinished: boolean;
  } | null {
    return streamSubscriptions.get(message.chatId) || null;
  }
</script>

<!-- In the message list rendering -->
{#each messages as message (message.id)}
  <!-- Check if message is streaming and subscribe if needed -->
  {#if message.isStreaming}
    {#await ensureStreamSubscription(message)}
      <!-- Loading state -->
    {:then}
      <!-- Subscription established -->
    {/await}
  {/if}

  <Message
    {message}
    streamContent={getStreamContent(message)}
    onEdit={handleEditMessage}
  />
{/each}
```

---

### 4. `frontend/src/lib/components/Message.svelte`

**Add streaming content rendering:**

```svelte
<script lang="ts">
  import type { Message } from '$lib/api/schemas';

  export let message: Message;
  export let streamContent: {
    reasoningContent: string;
    content: string;
    toolCalls: any[];
    isFinished: boolean;
  } | null = null;

  // Determine what content to display
  $: displayContent = streamContent && !streamContent.isFinished
    ? streamContent.content
    : message.content;

  $: displayReasoning = streamContent && !streamContent.isFinished
    ? streamContent.reasoningContent
    : message.reasoningContent;

  $: isStreaming = message.isStreaming && streamContent && !streamContent.isFinished;
</script>

<div class="message" class:streaming={isStreaming}>
  <div class="message-header">
    <span class="role-badge">{message.role}</span>
    <span class="timestamp">{new Date(message.createdAt).toLocaleString()}</span>
    {#if isStreaming}
      <span class="streaming-indicator">
        <span class="dot"></span>
        <span class="dot"></span>
        <span class="dot"></span>
      </span>
    {/if}
  </div>

  <!-- Reasoning content (collapsible) -->
  {#if displayReasoning}
    <details class="reasoning-section">
      <summary>Thinking</summary>
      <div class="reasoning-content">
        {displayReasoning}
      </div>
    </details>
  {/if}

  <!-- Main content -->
  <div class="message-content">
    {#if isStreaming && !displayContent}
      <span class="streaming-placeholder">Generating response...</span>
    {:else}
      {@html renderMarkdown(displayContent)}
    {/if}
  </div>

  <!-- Tool calls -->
  {#if streamContent && streamContent.toolCalls.length > 0}
    <div class="tool-calls">
      {#each streamContent.toolCalls as toolCall}
        <div class="tool-call">
          <span class="tool-name">{toolCall.name}</span>
          <pre class="tool-arguments">{toolCall.arguments}</pre>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Tags -->
  {#if message.tags.length > 0}
    <div class="tags">
      {#each message.tags as tag}
        <span class="tag">{tag}</span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .message.streaming {
    border-left: 3px solid var(--streaming-color, #3b82f6);
    padding-left: 12px;
  }

  .streaming-indicator {
    display: inline-flex;
    gap: 3px;
    margin-left: 8px;
  }

  .streaming-indicator .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background-color: var(--streaming-color, #3b82f6);
    animation: pulse 1.4s infinite;
  }

  .streaming-indicator .dot:nth-child(2) {
    animation-delay: 0.2s;
  }

  .streaming-indicator .dot:nth-child(3) {
    animation-delay: 0.4s;
  }

  @keyframes pulse {
    0%, 100% {
      opacity: 0.3;
      transform: scale(0.8);
    }
    50% {
      opacity: 1;
      transform: scale(1);
    }
  }

  .streaming-placeholder {
    color: var(--text-muted, #6b7280);
    font-style: italic;
  }

  .reasoning-section {
    margin-bottom: 12px;
    padding: 8px;
    background-color: var(--reasoning-bg, #f3f4f6);
    border-radius: 4px;
  }

  .reasoning-section summary {
    cursor: pointer;
    font-weight: 500;
    color: var(--text-secondary, #4b5563);
  }

  .reasoning-content {
    margin-top: 8px;
    white-space: pre-wrap;
    font-size: 0.9em;
    color: var(--text-secondary, #4b5563);
  }

  .tool-calls {
    margin-top: 12px;
    padding: 8px;
    background-color: var(--tool-calls-bg, #f9fafb);
    border-radius: 4px;
  }

  .tool-call {
    margin-bottom: 8px;
  }

  .tool-name {
    font-weight: 500;
    color: var(--tool-name-color, #059669);
  }

  .tool-arguments {
    margin-top: 4px;
    padding: 4px;
    background-color: var(--code-bg, #f3f4f6);
    border-radius: 2px;
    font-size: 0.85em;
    overflow-x: auto;
  }
</style>
```

---

### 5. `frontend/src/lib/components/ChatView.svelte` (Auto-scroll)

**Update auto-scroll logic to handle streaming:**

```svelte
<script lang="ts">
  // ... existing auto-scroll logic ...

  // Track if we should auto-scroll
  let shouldAutoScroll = true;
  let messageContainer: HTMLDivElement;

  // Auto-scroll when streaming content updates
  $: if (streamSubscriptions.size > 0 && shouldAutoScroll) {
    scrollToBottom();
  }

  function scrollToBottom() {
    if (messageContainer) {
      messageContainer.scrollTop = messageContainer.scrollHeight;
    }
  }

  function handleScroll() {
    if (!messageContainer) return;

    const { scrollTop, scrollHeight, clientHeight } = messageContainer;
    const isAtBottom = scrollHeight - scrollTop - clientHeight < 30;

    shouldAutoScroll = isAtBottom;
  }
</script>

<div
  bind:this={messageContainer}
  class="message-container"
  on:scroll={handleScroll}
>
  <!-- Message list -->
</div>
```

---

## Tests

### Unit Tests

1. **`test_message_schema_streaming_fields`**:
   - Verify `MessageSchema` correctly parses messages with `isFinished` and `isStreaming` fields.
   - Verify defaults: `isFinished: true, isStreaming: false`.

2. **`test_stream_subscribe_result_parsing`**:
   - Verify `StreamSubscribeResult` is correctly parsed from WebSocket response.

3. **`test_stream_chunk_event_handling`**:
   - Verify `streamChunk` events are correctly dispatched to handlers.
   - Verify different chunk types (reasoningDelta, contentDelta, toolCallDelta) are handled.

### Integration Tests

1. **`test_streaming_message_rendering`**:
   - Create a message with `isStreaming: true`.
   - Verify the message component shows the streaming indicator.
   - Simulate `streamChunk` events and verify content updates reactively.
   - Simulate `streamFinished` event and verify the indicator disappears.

2. **`test_stream_subscription_lifecycle`**:
   - Verify subscription is created when a streaming message appears.
   - Verify subscription is cleaned up after stream finishes.
   - Verify multiple streaming messages can be subscribed simultaneously.

---

## Implementation Notes

1. **Reactive state management**: The `streamSubscriptions` map is replaced with a new instance after each update to trigger Svelte's reactivity system. This is necessary because Svelte doesn't track mutations to Map contents.

2. **Auto-scroll**: The existing auto-scroll logic is reused. When streaming content updates, the view auto-scrolls to the bottom if the user is already at the bottom. If the user has scrolled up, auto-scroll is disabled until they scroll back to the bottom.

3. **Streaming indicator**: The animated dots indicator is shown when a message is streaming. This reuses the existing UI pattern from the old streaming implementation.

4. **Tool call rendering**: Tool calls are displayed as they arrive during streaming. The arguments are accumulated and displayed in a `<pre>` block.

5. **Error handling**: If `streamSubscribe` fails, the error is logged but the UI continues to function. The message will show the last known content (if any) or a placeholder.

6. **Cleanup**: Stream subscriptions are cleaned up 1 second after the stream finishes. This delay allows the final content to be rendered before the subscription is removed.

7. **Markdown rendering**: The existing `renderMarkdown` function is used to render the streaming content. This ensures consistency with non-streaming messages.
