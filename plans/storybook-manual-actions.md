# Storybook Manual Actions Implementation Plan

## Overview

Implement a declarative step-by-step execution system for Storybook stories using a `StoryExecutionControls` component. This allows manual testing of user flows by clicking through predefined steps.

## Architecture

### State Management Pattern

```typescript
interface StepDefinition<T> {
  name: string;
  execute: (options: { state: T }) => Promise<{ state: T }>;
}

interface StoryControlDefinition<T> {
  getInitialState: () => T;
  steps: StepDefinition<T>[];
}
```

- State is **immutable** - each step receives the current state and returns a new state
- Steps are executed sequentially, with state passed from one to the next
- The component tracks the current step index and execution status

### Component Behavior

1. `StoryExecutionControls` renders a single button showing the current step name
2. Button is **disabled** while a step is executing (async operation)
3. After execution completes, button updates to show the next step name
4. When all steps are complete, button shows "All steps completed" and is disabled

## Implementation Steps

### Step 1: Create Test IDs Constants File

**File**: `frontend/src/stories/testIds.ts`

Create a centralized file with all test ID constants to avoid duplicates and ensure consistency:

```typescript
export const TEST_IDS = {
  MESSAGE_INPUT: 'message-input',
  SEND_BUTTON: 'send-button',
} as const;
```

### Step 2: Add data-testid Attributes to MessageInput

**File**: `frontend/src/components/MessageInput.svelte`

Add test identifiers using the constants from `testIds.ts`:

```svelte
<script lang="ts">
  import { TEST_IDS } from '@/stories/testIds';
  // ... other imports
</script>

<textarea
  bind:this={textareaElement}
  bind:value={input}
  on:keydown={handleKeydown}
  on:input={handleInput}
  placeholder={$isPaused || $isAborted ? 'Type a message to queue...' : 'Type a message...'}
  disabled={!canSend && !canQueue}
  class="input-textarea"
  rows="1"
  data-testid={TEST_IDS.MESSAGE_INPUT}
></textarea>

<button 
  on:click={handleSend} 
  disabled={!input.trim() || !$currentChatId || hasMcpError} 
  class="send-btn"
  data-testid={TEST_IDS.SEND_BUTTON}
>
  {canQueue ? 'Queue' : 'Send'}
</button>
```

### Step 3: Extend Mock WebSocket with Event Emission

**File**: `frontend/src/stories/mockWs.ts`

Add functions to emit WebSocket events for testing:

```typescript
import type { WsResponse } from '@/lib/types/ws';

type RequestHandler = (request: Record<string, unknown>) => WsResponse | Promise<WsResponse>;
type EventHandler = (data: any) => void;

const handlers = new Map<string, RequestHandler>();
const eventHandlers = new Map<string, EventHandler[]>();
let defaultHandler: RequestHandler = () => ({ type: 'response', id: '', success: true, data: {} });

export function setMockWsHandler(requestType: string, handler: RequestHandler): void {
  handlers.set(requestType, handler);
}

export function setDefaultMockWsHandler(handler: RequestHandler): void {
  defaultHandler = handler;
}

export function clearMockWsHandlers(): void {
  handlers.clear();
  eventHandlers.clear();
  defaultHandler = () => ({ type: 'response', id: '', success: true, data: {} });
}

// Register event handler
export function onWsEvent(eventType: string, handler: EventHandler): void {
  if (!eventHandlers.has(eventType)) {
    eventHandlers.set(eventType, []);
  }
  eventHandlers.get(eventType)!.push(handler);
}

// Emit WebSocket event (for testing)
export function emitWsEvent(eventType: string, data: any): void {
  const handlers = eventHandlers.get(eventType);
  if (handlers) {
    handlers.forEach(handler => handler(data));
  }
}

// ... rest of existing code
```

### Step 4: Create Utility Functions

**File**: `frontend/src/stories/testUtils.ts`

Create utility functions for testing:

```typescript
/**
 * Async sleep function for testing
 */
export function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}
```

### Step 5: Create StoryExecutionControls Component

**File**: `frontend/src/stories/StoryExecutionControls.svelte`

```svelte
<script lang="ts">
  export interface StepDefinition<T> {
    name: string;
    execute: (options: { state: T }) => Promise<{ state: T }>;
  }
  
  export interface StoryControlDefinition<T> {
    getInitialState: () => T;
    steps: StepDefinition<T>[];
  }
  
  let { story }: { story: StoryControlDefinition<any> } = $props();
  
  let currentState = $state(story.getInitialState());
  let currentStepIndex = $state(0);
  let isExecuting = $state(false);
  let error = $state<string | null>(null);
  
  $: currentStep = story.steps[currentStepIndex];
  $: isComplete = currentStepIndex >= story.steps.length;
  
  async function executeStep() {
    if (!currentStep || isExecuting || isComplete) return;
    
    isExecuting = true;
    error = null;
    
    try {
      const result = await currentStep.execute({ state: currentState });
      currentState = result.state;
      currentStepIndex += 1;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Unknown error';
      console.error('Step execution failed:', err);
    } finally {
      isExecuting = false;
    }
  }
</script>

<div class="story-controls">
  <div class="controls-header">
    <h3>Story Execution Controls</h3>
  </div>
  
  {#if error}
    <div class="error-message">{error}</div>
  {/if}
  
  <div class="step-info">
    <span>Step {currentStepIndex + 1} of {story.steps.length}</span>
    {#if isComplete}
      <span class="complete">✓ All steps completed</span>
    {/if}
  </div>
  
  <button
    on:click={executeStep}
    disabled={isExecuting || isComplete}
    class="execute-btn"
  >
    {#if isExecuting}
      Executing...
    {:else if isComplete}
      All steps completed
    {:else}
      {currentStep.name}
    {/if}
  </button>
</div>

<style>
  .story-controls {
    padding: 16px;
    border: 1px solid #ddd;
    border-radius: 8px;
    background: #f9f9f9;
    margin-bottom: 16px;
  }
  
  .controls-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }
  
  .controls-header h3 {
    margin: 0;
    font-size: 16px;
  }
  
  .step-info {
    display: flex;
    justify-content: space-between;
    margin-bottom: 12px;
    font-size: 14px;
    color: #666;
  }
  
  .complete {
    color: #28a745;
    font-weight: bold;
  }
  
  .execute-btn {
    width: 100%;
    padding: 12px;
    font-size: 14px;
    font-weight: 500;
    background: #007bff;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    transition: background 0.2s;
  }
  
  .execute-btn:hover:not(:disabled) {
    background: #0056b3;
  }
  
  .execute-btn:disabled {
    background: #ccc;
    cursor: not-allowed;
  }
  
  .error-message {
    background: #fee;
    border: 1px solid #fcc;
    color: #c33;
    padding: 8px;
    border-radius: 4px;
    margin-bottom: 12px;
    font-size: 13px;
  }
</style>
```

### Step 6: Update ChatView.stories.ts

**File**: `frontend/src/stories/pause-abort/ChatView.stories.ts`

Remove existing mock messages and implement the new pattern:

```typescript
import type { Meta, StoryObj } from '@storybook/svelte';
import ChatView from '@/components/ChatView.svelte';
import StoryExecutionControls from '@/stories/StoryExecutionControls.svelte';
import type { StoryControlDefinition } from '@/stories/StoryExecutionControls.svelte';
import { 
  chats, 
  currentChatId, 
  messages, 
  availableModels, 
  selectedModel, 
  isStreaming, 
  streamError, 
  streamingMessageId, 
  isPaused, 
  pendingToolCalls, 
  availableRoles, 
  activeRole, 
  todoList, 
  resetAllStores 
} from '@/lib/chatStores';
import { chatProjects, mcpStatuses } from '@/lib/projectStores';
import { setMockWsHandler, clearMockWsHandlers, emitWsEvent } from '@/stories/mockWs';
import { TEST_IDS } from '@/stories/testIds';
import { sleep } from '@/stories/testUtils';

const mockChat = {
  id: 1,
  title: 'Test Chat',
  createdAt: '2026-07-23T10:00:00Z',
  updatedAt: '2026-07-23T10:00:00Z',
  activeModel: 'gpt-4',
};

interface StoryState {
  messageId: number;
}

function setupDefaultStores() {
  resetAllStores();
  chats.set([mockChat]);
  currentChatId.set(1);
  messages.set([]); // Start with empty messages
  availableModels.set(['gpt-4', 'gpt-3.5-turbo']);
  selectedModel.set('gpt-4');
  isStreaming.set(false);
  streamError.set(null);
  streamingMessageId.set(null);
  isPaused.set(false);
  pendingToolCalls.set([]);
  availableRoles.set([]);
  activeRole.set(null);
  todoList.set([]);
  chatProjects.set([]);
  mcpStatuses.set([]);
}

const storyDefinition: StoryControlDefinition<StoryState> = {
  getInitialState: () => ({
    messageId: 1,
  }),
  
  steps: [
    {
      name: 'Add user message',
      execute: async ({ state }) => {
        // Simulate user typing and sending a message
        const textarea = document.querySelector(`[data-testid="${TEST_IDS.MESSAGE_INPUT}"]`) as HTMLTextAreaElement;
        const sendButton = document.querySelector(`[data-testid="${TEST_IDS.SEND_BUTTON}"]`) as HTMLButtonElement;
        
        if (!textarea || !sendButton) {
          throw new Error('Could not find message input or send button');
        }
        
        // Set value and trigger input event
        textarea.value = 'Hello, how are you?';
        textarea.dispatchEvent(new Event('input', { bubbles: true }));
        
        // Wait a bit for UI to update
        await sleep(100);
        
        // Click send button
        sendButton.click();
        
        // Wait for message to be added
        await sleep(200);
        
        return { state: { ...state, messageId: state.messageId + 1 } };
      },
    },
    
    {
      name: 'Receive AI response',
      execute: async ({ state }) => {
        // Set up mock handler for sendMessage
        setMockWsHandler('sendMessage', async (request) => {
          // Simulate streaming response using async/await
          await sleep(100);
          emitWsEvent('chatStreamChunk', {
            chatId: 1,
            content: 'I\'m doing well, thank you! ',
          });
          
          await sleep(100);
          emitWsEvent('chatStreamChunk', {
            chatId: 1,
            content: 'How can I help you today?',
          });
          
          await sleep(100);
          emitWsEvent('chatStreamFinished', {
            chatId: 1,
          });
          
          return { type: 'response', id: request.id as string, success: true, data: {} };
        });
        
        // Wait for streaming to complete
        await sleep(500);
        
        clearMockWsHandlers();
        
        return { state: { ...state, messageId: state.messageId + 1 } };
      },
    },
  ],
};

setupDefaultStores();

const meta = {
  title: 'Pause-Abort/ChatView',
  component: ChatView,
  tags: ['autodocs'],
  parameters: {
    layout: 'fullscreen',
  },
  decorators: [
    (Story) => {
      setupDefaultStores();
      return Story();
    },
  ],
} satisfies Meta<ChatView>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const WithControls: Story = {
  render: () => ({
    components: { StoryExecutionControls, ChatView },
    props: {
      story: storyDefinition,
    },
    template: `
      <div style="display: flex; flex-direction: column; height: 100vh;">
        <StoryExecutionControls {story} />
        <div style="flex: 1; overflow: hidden;">
          <ChatView />
        </div>
      </div>
    `,
  }),
};
```

### Step 7: Update Memory Documentation

**File**: `memory/frontend.md`

Add a note about the import convention for all frontend code:

```markdown
## Import Convention

All imports in frontend code should use the `@/` alias instead of relative paths (`../`). This ensures consistency and makes imports more maintainable across the entire codebase.

**Correct:**
```typescript
import { TEST_IDS } from '@/stories/testIds';
import StoryExecutionControls from '@/stories/StoryExecutionControls.svelte';
import { dispatch } from '@/lib/actions';
```

**Incorrect:**
```typescript
import { TEST_IDS } from '../testIds';
import StoryExecutionControls from '../StoryExecutionControls.svelte';
import { dispatch } from '../lib/actions';
```

This convention applies to:
- Components
- Stories
- Tests
- Utilities
- Any other frontend code

### Test IDs

Test IDs are centralized in `frontend/src/stories/testIds.ts` to prevent duplicates and ensure consistency across components and tests.
```

## Key Design Decisions

1. **Immutable State**: Each step receives the current state and returns a new state object, ensuring predictability and enabling potential undo/redo functionality.

2. **Async Execution**: Steps are async to support DOM interactions and waiting for WebSocket events.

3. **Error Handling**: Errors during step execution are caught and displayed in the UI, allowing debugging without breaking the entire story.

4. **Test IDs**: Using centralized `TEST_IDS` constants ensures consistency and prevents duplicates.

5. **Mock WebSocket Extension**: Extended `mockWs.ts` with `emitWsEvent()` function to properly simulate WebSocket events without relying on DOM events.

6. **Async Sleep Utility**: Created `sleep()` function for cleaner async code instead of using `setTimeout` directly.

7. **Import Convention**: All frontend code uses `@/` imports instead of relative paths for better maintainability.

## Future Enhancements

- Add ability to jump to specific steps
- Add step history/log display
- Support for conditional steps based on state
- Add ability to export/import state for debugging
