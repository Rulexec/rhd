# Phase 4: Frontend Integration

## Overview

This phase updates the frontend to handle new pause/abort states, show appropriate UI, and support message queuing. The frontend will display pause/abort indicators, allow message input when paused/aborted, show queued message indicators, and handle streaming message removal on abort.

**Scope:**
- Add `queuedMessages` store to track queued messages
- Update `isPaused` logic to handle both paused and aborted states
- Add `isAborted` store to distinguish between paused and aborted
- Add `queueMessage()` operation to send queue requests to daemon
- Add handlers for `StreamAborted` and `MessageQueued` events
- Update `MessageInput` component to allow input when paused/aborted
- Update `Message` component to handle streaming message removal on abort
- Show "Queued" indicator for queued messages

**Out of scope:**
- Test case creation (Phase 5)
- E2E test implementation (Phase 6)

**Dependencies:**
- Phase 1 (Backend State Machine) must be completed first
- Phase 2 (Tool Loop Integration) must be completed first
- Phase 3 (Message Queue) must be completed first

## Files to Modify

### 1. `frontend/src/lib/chatStores.ts`

**Add `queuedMessages` store:**

```typescript
import type { Chat, ChatMessage, ToolCall, RoleInfo, ActiveRole, TodoItem, QueuedMessage } from './types/index';

export const queuedMessages: Writable<QueuedMessage[]> = writable([]);
```

**Add `isAborted` store:**

```typescript
export const isAborted: Writable<boolean> = writable(false);
```

**Update `resetAllStores()` to reset new stores:**

```typescript
export function resetAllStores(): void {
  chats.set([]);
  currentChatId.set(null);
  messages.set([]);
  streamingContent.set('');
  streamingThinkingContent.set('');
  isStreaming.set(false);
  streamError.set(null);
  availableModels.set([]);
  selectedModel.set(null);
  streamingMessageId.set(null);
  isPaused.set(false);
  isAborted.set(false);
  pendingToolCalls.set([]);
  queuedMessages.set([]);
  availableRoles.set([]);
  activeRole.set(null);
  todoList.set([]);
}
```

**Update `isToolLoopRunning` derived store:**

```typescript
export const isToolLoopRunning: Readable<boolean> = derived(
  [isStreaming, isPaused],
  ([$isStreaming, $isPaused]) => $isStreaming && !$isPaused
);
```

### 2. `frontend/src/lib/types/index.ts`

**Add `QueuedMessage` type:**

```typescript
export interface QueuedMessage {
  id: string;
  content: string;
  model: string;
  queuedAt: string;
  status: 'queued';
}
```

### 3. `frontend/src/lib/chatWs/operations.ts`

**Add `queueMessage()` operation:**

```typescript
export async function queueMessage(content: string, model: string): Promise<WsResponse> {
  const chatId = get(currentChatId);
  if (!chatId) return { id: '', type: 'response', success: false, error: 'No chat selected' };

  const tempId = `queued-${Date.now()}`;
  const queuedMessage: QueuedMessage = {
    id: tempId,
    content,
    model,
    queuedAt: new Date().toISOString(),
    status: 'queued',
  };
  
  queuedMessages.update((list) => [...list, queuedMessage]);

  const id = generateRequestId();
  const response = await sendRequest({ type: 'queueMessage', id, chatId, content, model });
  if (!response.success) {
    // Remove from queue if request failed
    queuedMessages.update((list) => list.filter((m) => m.id !== tempId));
  }
  return response;
}
```

**Update `pauseChat()` to set isPaused and isAborted:**

```typescript
export async function pauseChat(): Promise<WsResponse> {
  const chatId = get(currentChatId);
  if (!chatId) return { id: '', type: 'response', success: false, error: 'No chat selected' };

  const id = generateRequestId();
  const response = await sendRequest({ type: 'pauseChat', id, chatId });
  return response;
}
```

**Update `abortChat()` to set isPaused and isAborted:**

```typescript
export async function abortChat(): Promise<WsResponse> {
  const chatId = get(currentChatId);
  if (!chatId) return { id: '', type: 'response', success: false, error: 'No chat selected' };

  const id = generateRequestId();
  const response = await sendRequest({ type: 'abortChat', id, chatId });
  return response;
}
```

**Update `resumeChat()` to clear isPaused and isAborted:**

```typescript
export async function resumeChat(): Promise<WsResponse> {
  const chatId = get(currentChatId);
  if (!chatId) return { id: '', type: 'response', success: false, error: 'No chat selected' };

  const id = generateRequestId();
  const response = await sendRequest({ type: 'resumeChat', id, chatId });
  return response;
}
```

### 4. `frontend/src/lib/chatWs/events.ts`

**Add handler for `streamAborted` event:**

```typescript
case 'streamAborted': {
  // Remove streaming message from UI
  const tempId = get(streamingMessageId);
  if (tempId) {
    messages.update((list) => list.filter((m) => m.id !== tempId));
    streamingMessageId.set(null);
  }
  
  // Set paused and aborted states
  isPaused.set(true);
  isAborted.set(true);
  isStreaming.set(false);
  streamingThinkingContent.set('');
  break;
}
```

**Add handler for `messageQueued` event:**

```typescript
case 'messageQueued': {
  const { content, model } = data as { content: string; model: string };
  const tempId = `queued-${Date.now()}`;
  const queuedMessage: QueuedMessage = {
    id: tempId,
    content,
    model,
    queuedAt: new Date().toISOString(),
    status: 'queued',
  };
  queuedMessages.update((list) => [...list, queuedMessage]);
  break;
}
```

**Update `chatPaused` handler:**

```typescript
case 'chatPaused': {
  isPaused.set(true);
  isAborted.set(false);
  isStreaming.set(false);
  break;
}
```

**Update `chatResumed` handler:**

```typescript
case 'chatResumed': {
  isPaused.set(false);
  isAborted.set(false);
  isStreaming.set(true);
  queuedMessages.set([]);
  break;
}
```

### 5. `frontend/src/lib/components/MessageInput.svelte`

**Update to allow input when paused/aborted:**

```svelte
<script lang="ts">
  import { isStreaming, isPaused, isAborted, selectedModel, queuedMessages } from '../chatStores';
  import { dispatch } from '../actions';
  
  $: canSend = $selectedModel && !$isStreaming;
  $: canQueue = $selectedModel && ($isPaused || $isAborted);
  $: showPauseButton = $isStreaming && !$isPaused;
  $: showAbortButton = $isStreaming && !$isPaused;
  $: showResumeButton = $isPaused || $isAborted;
  
  function handleSend() {
    if (canQueue) {
      // Queue message instead of sending
      dispatch({ type: 'queueMessage', payload: { content: inputValue, model: $selectedModel! } });
      inputValue = '';
    } else if (canSend) {
      dispatch({ type: 'sendMessage', payload: { content: inputValue, model: $selectedModel! } });
      inputValue = '';
    }
  }
</script>

<div class="message-input">
  <textarea
    bind:value={inputValue}
    on:keydown={handleKeydown}
    placeholder={$isPaused || $isAborted ? 'Type a message to queue...' : 'Type a message...'}
    disabled={!canSend && !canQueue}
  ></textarea>
  
  <div class="buttons">
    {#if showPauseButton}
      <button on:click={() => dispatch({ type: 'pauseChat' })}>Pause</button>
    {/if}
    
    {#if showAbortButton}
      <button on:click={() => dispatch({ type: 'abortChat' })} class="abort">Abort</button>
    {/if}
    
    {#if showResumeButton}
      <button on:click={() => dispatch({ type: 'resumeChat' })} class="resume">Resume</button>
    {/if}
    
    {#if canSend || canQueue}
      <button on:click={handleSend}>
        {canQueue ? 'Queue' : 'Send'}
      </button>
    {/if}
  </div>
  
  {#if $queuedMessages.length > 0}
    <div class="queued-indicator">
      {$queuedMessages.length} message{$queuedMessages.length === 1 ? '' : 's'} queued
    </div>
  {/if}
</div>
```

### 6. `frontend/src/lib/components/Message.svelte`

**Update to show "Queued" indicator for queued messages:**

```svelte
<script lang="ts">
  import type { ChatMessage, QueuedMessage } from '../types/index';
  
  export let message: ChatMessage | QueuedMessage;
  
  $: isQueued = 'status' in message && message.status === 'queued';
</script>

<div class="message" class:queued={isQueued}>
  {#if isQueued}
    <div class="queued-badge">Queued</div>
  {/if}
  
  <div class="content">
    {message.content}
  </div>
</div>

<style>
  .queued {
    opacity: 0.7;
  }
  
  .queued-badge {
    display: inline-block;
    padding: 2px 8px;
    background: #f0f0f0;
    border-radius: 4px;
    font-size: 12px;
    color: #666;
    margin-bottom: 4px;
  }
</style>
```

### 7. `frontend/src/lib/components/MessageList.svelte`

**Update to display queued messages:**

```svelte
<script lang="ts">
  import { messages, queuedMessages } from '../chatStores';
  import Message from './Message.svelte';
  
  $: allMessages = [...$messages, ...$queuedMessages];
</script>

<div class="message-list">
  {#each allMessages as message (message.id)}
    <Message {message} />
  {/each}
</div>
```

### 8. `frontend/src/lib/actions/types.ts`

**Add `queueMessage` action type:**

```typescript
export type ChatAction =
  | { type: 'createChat'; payload: { title: string } }
  | { type: 'selectChat'; payload: { chatId: number } }
  | { type: 'deleteChat'; payload: { chatId: number } }
  | { type: 'sendMessage'; payload: { content: string; model: string } }
  | { type: 'queueMessage'; payload: { content: string; model: string } }
  | { type: 'editMessage'; payload: { messageId: number; newContent: string; model: string } }
  | { type: 'abortChat' }
  | { type: 'pauseChat' }
  | { type: 'resumeChat' }
  // ... other action types
```

### 9. `frontend/src/lib/actions/processors.ts`

**Add processor for `queueMessage` action:**

```typescript
import { queueMessage } from '../chatWs/operations';

export async function processAction(action: ChatAction): Promise<void> {
  switch (action.type) {
    case 'queueMessage':
      await queueMessage(action.payload.content, action.payload.model);
      break;
    // ... other cases
  }
}
```

### 10. `frontend/src/lib/types/ws.ts`

**Add `QueueMessage` request type:**

```typescript
export const WsRequestSchema = z.discriminatedUnion('type', [
  // ... existing request types
  
  z.object({
    type: z.literal('queueMessage'),
    id: z.string(),
    chatId: z.number(),
    content: z.string(),
    model: z.string(),
  }),
  
  // ... other request types
]);
```

**Add `StreamAborted` and `MessageQueued` event types:**

```typescript
export const WsEventSchema = z.discriminatedUnion('event', [
  // ... existing event types
  
  z.object({
    event: z.literal('streamAborted'),
    data: z.object({
      chatId: z.number(),
    }),
  }),
  
  z.object({
    event: z.literal('messageQueued'),
    data: z.object({
      chatId: z.number(),
      content: z.string(),
      model: z.string(),
    }),
  }),
  
  // ... other event types
]);
```

## Tests

### Unit Tests

Add to `frontend/src/tests/ui/MessageInput.test.ts`:

```typescript
import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import MessageInput from '../../lib/components/MessageInput.svelte';
import { isStreaming, isPaused, isAborted, selectedModel, resetAllStores } from '../../lib/chatStores';

describe('MessageInput', () => {
  beforeEach(() => {
    resetAllStores();
  });

  it('renders pause button when streaming and not paused', () => {
    isStreaming.set(true);
    isPaused.set(false);
    selectedModel.set('gpt-4');
    
    render(MessageInput);
    
    expect(screen.getByText('Pause')).toBeInTheDocument();
    expect(screen.getByText('Abort')).toBeInTheDocument();
    expect(screen.queryByText('Resume')).not.toBeInTheDocument();
  });

  it('renders resume button when paused', () => {
    isStreaming.set(false);
    isPaused.set(true);
    isAborted.set(false);
    selectedModel.set('gpt-4');
    
    render(MessageInput);
    
    expect(screen.queryByText('Pause')).not.toBeInTheDocument();
    expect(screen.queryByText('Abort')).not.toBeInTheDocument();
    expect(screen.getByText('Resume')).toBeInTheDocument();
  });

  it('renders resume button when aborted', () => {
    isStreaming.set(false);
    isPaused.set(true);
    isAborted.set(true);
    selectedModel.set('gpt-4');
    
    render(MessageInput);
    
    expect(screen.queryByText('Pause')).not.toBeInTheDocument();
    expect(screen.queryByText('Abort')).not.toBeInTheDocument();
    expect(screen.getByText('Resume')).toBeInTheDocument();
  });

  it('allows message input when paused', async () => {
    isStreaming.set(false);
    isPaused.set(true);
    selectedModel.set('gpt-4');
    
    render(MessageInput);
    
    const textarea = screen.getByPlaceholderText('Type a message to queue...');
    expect(textarea).not.toBeDisabled();
    
    await fireEvent.input(textarea, { target: { value: 'Hello' } });
    expect(textarea).toHaveValue('Hello');
  });

  it('shows queue button when paused', () => {
    isStreaming.set(false);
    isPaused.set(true);
    selectedModel.set('gpt-4');
    
    render(MessageInput);
    
    expect(screen.getByText('Queue')).toBeInTheDocument();
  });

  it('shows queued messages indicator', () => {
    isPaused.set(true);
    selectedModel.set('gpt-4');
    queuedMessages.set([
      { id: 'queued-1', content: 'Hello', model: 'gpt-4', queuedAt: new Date().toISOString(), status: 'queued' },
      { id: 'queued-2', content: 'World', model: 'gpt-4', queuedAt: new Date().toISOString(), status: 'queued' },
    ]);
    
    render(MessageInput);
    
    expect(screen.getByText('2 messages queued')).toBeInTheDocument();
  });
});
```

Add to `frontend/src/tests/ui/Message.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import Message from '../../lib/components/Message.svelte';

describe('Message', () => {
  it('shows Queued indicator for queued messages', () => {
    const queuedMessage = {
      id: 'queued-1',
      content: 'Hello',
      model: 'gpt-4',
      queuedAt: new Date().toISOString(),
      status: 'queued' as const,
    };
    
    render(Message, { message: queuedMessage });
    
    expect(screen.getByText('Queued')).toBeInTheDocument();
  });

  it('does not show Queued indicator for regular messages', () => {
    const regularMessage = {
      id: 1,
      chatId: 1,
      role: 'user' as const,
      content: 'Hello',
      createdAt: new Date().toISOString(),
      model: 'gpt-4',
    };
    
    render(Message, { message: regularMessage });
    
    expect(screen.queryByText('Queued')).not.toBeInTheDocument();
  });
});
```

Add to `frontend/src/tests/ui/ToolCall.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import ToolCallMessage from '../../lib/components/ToolCallMessage.svelte';

describe('ToolCallMessage', () => {
  it('shows Aborted error for cancelled tool calls', () => {
    const toolCall = {
      id: 'call_1',
      name: 'test_tool',
      arguments: '{}',
      status: 'failed' as const,
      result: 'Aborted',
      mcpId: 'test_mcp',
    };
    
    render(ToolCallMessage, { toolCall });
    
    expect(screen.getByText('Aborted')).toBeInTheDocument();
  });
});
```

### Integration Tests

Add to `frontend/src/tests/e2e/chat-state.test.ts`:

```typescript
import { describe, it, expect } from 'vitest';
import { test } from '../../testUtils';

describe('Chat State - Pause/Abort/Resume', () => {
  it('pauses during AI call and resumes with pending tool calls', async ({ page }) => {
    // Step 2-7: Pause phase
    // Step 10-16: Message queue phase
    // Step 17-29: Resume phase
  });

  it('pauses during tool execution and resumes', async ({ page }) => {
    // Step 2-10: Pause phase
    // Step 11-17: Message queue phase
    // Step 18-28: Resume phase
  });

  it('aborts during AI call and resumes without aborted message', async ({ page }) => {
    // Step 2-12: Abort phase
    // Step 13-19: Message queue phase
    // Step 20-30: Resume phase
  });

  it('aborts during tool execution and resumes with error results', async ({ page }) => {
    // Step 2-13: Abort phase
    // Step 14-20: Message queue phase
    // Step 21-31: Resume phase
  });

  it('queues multiple messages during pause and sends on resume', async ({ page }) => {
    // Step 3-7: Queue first message
    // Step 8-14: Queue second message
    // Step 15-26: Resume
  });
});
```

## Implementation Notes

1. **State representation**: The frontend uses two boolean stores to represent the pause/abort state:
   - `isPaused: boolean` — true when paused OR aborted
   - `isAborted: boolean` — true only when aborted (subset of isPaused)
   
   This allows the UI to show different messages for paused vs aborted states while treating them similarly for most operations.

2. **UI behavior**:
   - When paused/aborted: show Resume button, allow message input
   - Queued messages show "Queued" indicator
   - On abort during streaming: remove streaming message from UI
   - On resume: clear queued messages store

3. **Message flow**:
   - User sends message while paused → `queueMessage()` instead of `sendMessage()`
   - On resume → backend processes queued messages automatically
   - Frontend clears `queuedMessages` store on `chatResumed` event

4. **Streaming message removal**: When `streamAborted` event is received, the frontend removes the streaming message (identified by `streamingMessageId`) from the messages list. This ensures the user doesn't see a partial message from the aborted AI call.

5. **Queued message indicator**: The `MessageInput` component shows a count of queued messages below the input field. This provides visual feedback that messages are being queued.

6. **Button visibility**: The pause/abort/resume buttons are shown based on the current state:
   - `showPauseButton = isStreaming && !isPaused`
   - `showAbortButton = isStreaming && !isPaused`
   - `showResumeButton = isPaused || isAborted`

7. **Input placeholder**: The input field placeholder changes based on the state:
   - Normal: "Type a message..."
   - Paused/Aborted: "Type a message to queue..."

8. **Send vs Queue button**: The send button text changes based on the state:
   - Normal: "Send"
   - Paused/Aborted: "Queue"

## Dependencies

- This phase depends on: Phase 1 (Backend State Machine), Phase 2 (Tool Loop Integration), Phase 3 (Message Queue)
- This phase must be completed before: Phase 5 (Test Case Creation)
- Phase 6 (E2E Test Implementation) can start after this phase
