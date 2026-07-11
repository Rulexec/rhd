# Frontend Actions-Based Testing Paradigm

## Status: COMPLETED

## Overview

Introduce actions concept to frontend architecture. UI components emit actions. ActionDispatcher routes actions to processors or test overrides. Tests intercept actions at dispatcher level, read payloads, and dispatch response actions.

---

## Current State Analysis

### Current Architecture (Problematic)

```
┌─────────────────┐
│   Component     │
│  (MessageInput) │
└────────┬────────┘
         │ directly calls
         ▼
┌─────────────────┐
│   chatWs.ts     │
│  (sendMessage)  │
└────────┬────────┘
         │ calls
         ▼
┌─────────────────┐
│    ws.ts        │
│  (sendRequest)  │
└────────┬────────┘
         │ WebSocket
         ▼
┌─────────────────┐
│    Daemon       │
└─────────────────┘
```

**Issues:**
1. Components tightly coupled to `chatWs.ts` functions
2. Hard to intercept external communication for testing
3. State logic mixed with UI triggers
4. Tests must render UI to test state changes

### Existing Tests

| File | Approach | Status |
|------|----------|--------|
| `chat-state.test.ts` | State-based (no UI) | ✅ Keep |
| `chat.test.ts` | UI-based | ❌ Remove/rewrite |
| `chat-messageflow.test.ts` | UI-based | ❌ Remove/rewrite |
| `chat-streaming.test.ts` | UI-based | ❌ Remove/rewrite |
| `projects.test.ts` | UI-based | ❌ Remove/rewrite |
| `scenarios.test.ts` | UI-based | ❌ Remove/rewrite |

---

## New Architecture

### Actions Concept

```
┌─────────────────┐
│   Component     │
│  (MessageInput) │
└────────┬────────┘
         │ dispatch({ type: 'sendMessage', payload })
         ▼
┌─────────────────────────────────────┐
│         ActionDispatcher            │
│  ┌─────────────────────────────┐    │
│  │ Check for test override     │────┼──► If override exists, call it
│  └─────────────────────────────┘    │
│  ┌─────────────────────────────┐    │
│  │ Call action processor       │────┼──► Default behavior
│  └─────────────────────────────┘    │
└─────────────────────────────────────┘
         │
         ▼
┌─────────────────┐
│ActionProcessor  │
│ (sendMessage)   │
└────────┬────────┘
         │ updates stores / calls external
         ▼
┌─────────────────┐
│    ws.ts        │
│  (sendRequest)  │
└────────┬────────┘
         │ WebSocket
         ▼
┌─────────────────┐
│    Daemon       │
└─────────────────┘
```

### Key Principles

1. **Components emit actions** — UI triggers `dispatch({ type: 'sendMessage', payload })` instead of calling `sendMessage()` directly
2. **ActionDispatcher routes** — Checks for test overrides first, then calls processors
3. **Test overrides intercept** — `_testOverrideAction('sendMessage', handler)` replaces processor for that action
4. **Handlers dispatch response actions** — Test handlers can dispatch follow-up actions (e.g., `chatMessageAdded`)
5. **Test cases human-readable** — Separate markdown files describe user scenarios

---

## Implementation Plan

### Phase 1: Create Human-Readable Test Cases

**Location:** `tests/cases/`

**Structure:**
```
tests/
├── cases/
│   ├── chat-create.md
│   ├── chat-send-message.md
│   ├── chat-streaming.md
│   ├── chat-edit-message.md
│   ├── chat-abort.md
│   ├── chat-pause-resume.md
│   └── projects-attach.md
└── README.md
```

**Example test case (`chat-send-message.md`):**
```markdown
# Test Case: Send Message

## Description
User sends a message in chat and receives AI response.

## Preconditions
- Chat exists and is selected
- Model is selected
- WebSocket connected

## Steps
1. User types message in input field
2. User clicks Send button (or presses Enter)
3. System dispatches `sendMessage` action
4. System shows streaming indicator
5. Daemon sends response chunks (or test dispatches `chatStreamChunk` actions)
6. System displays response
7. Streaming completes (test dispatches `chatStreamFinished`)

## Expected Results
- Message appears in message list
- Streaming indicator shown during response
- Response content displayed
- Input field re-enabled after completion

## Covered By
- `frontend/src/tests/e2e/chat-state.test.ts` (state logic)
- `frontend/src/tests/ui/MessageInput.test.ts` (UI rendering)
```

### Phase 2: Introduce Actions Layer

**New files:**
- `frontend/src/lib/actions/types.ts` — Action type definitions
- `frontend/src/lib/actions/dispatcher.ts` — ActionDispatcher with override mechanism
- `frontend/src/lib/actions/processors.ts` — Action processors (wrap existing chatWs functions)
- `frontend/src/lib/actions/index.ts` — Public exports

**Action types:**
```typescript
// actions/types.ts
export type ChatAction =
  // User actions (from UI)
  | { type: 'createChat'; payload: { title: string } }
  | { type: 'selectChat'; payload: { chatId: number } }
  | { type: 'deleteChat'; payload: { chatId: number } }
  | { type: 'sendMessage'; payload: { content: string; model: string } }
  | { type: 'editMessage'; payload: { messageId: number; content: string; model: string } }
  | { type: 'abortChat' }
  | { type: 'pauseChat' }
  | { type: 'resumeChat' }
  | { type: 'selectModel'; payload: { model: string } }
  // System actions (from WebSocket or tests)
  | { type: 'chatStreamChunk'; payload: { content: string } }
  | { type: 'chatStreamFinished' }
  | { type: 'chatStreamError'; payload: { error: string } }
  | { type: 'chatMessageAdded'; payload: { message: ChatMessage } };
```

**ActionDispatcher with override mechanism:**
```typescript
// actions/dispatcher.ts
import type { ChatAction } from './types';
import { processAction } from './processors';

type ActionHandler = (action: ChatAction) => void | Promise<void>;
type ActionOverride = (action: ChatAction) => void | Promise<void>;

// Map of action type -> override handler
const overrides = new Map<string, ActionOverride>();

// Test-only: override specific action
export function _testOverrideAction(actionType: string, handler: ActionOverride): void {
  overrides.set(actionType, handler);
}

// Test-only: clear all overrides
export function _testClearOverrides(): void {
  overrides.clear();
}

// Main dispatch function
export async function dispatch(action: ChatAction): Promise<void> {
  // Check for test override first
  const override = overrides.get(action.type);
  if (override) {
    await override(action);
    return;
  }
  
  // Default: call processor
  await processAction(action);
}
```

**Action processors:**
```typescript
// actions/processors.ts
import type { ChatAction } from './types';
import * as chatWs from '../chatWs';
import { handleChatEvent } from '../chatWs';

export async function processAction(action: ChatAction): Promise<void> {
  switch (action.type) {
    case 'createChat':
      await chatWs.createChat(action.payload.title);
      break;
    case 'selectChat':
      await chatWs.selectChat(action.payload.chatId);
      break;
    case 'deleteChat':
      await chatWs.deleteChat(action.payload.chatId);
      break;
    case 'sendMessage':
      await chatWs.sendMessage(action.payload.content, action.payload.model);
      break;
    case 'editMessage':
      await chatWs.editMessage(action.payload.messageId, action.payload.content, action.payload.model);
      break;
    case 'abortChat':
      await chatWs.abortChat();
      break;
    case 'pauseChat':
      await chatWs.pauseChat();
      break;
    case 'resumeChat':
      await chatWs.resumeChat();
      break;
    // System actions - delegate to handleChatEvent
    case 'chatStreamChunk':
    case 'chatStreamFinished':
    case 'chatStreamError':
    case 'chatMessageAdded':
      handleChatEvent(action.type, action.payload);
      break;
  }
}
```

### Phase 3: Refactor Components to Use Actions

**Changes to `MessageInput.svelte`:**
```svelte
<script lang="ts">
  import { dispatch } from '../lib/actions';
  
  function send() {
    if (!input.trim() || ...) return;
    dispatch({ type: 'sendMessage', payload: { content: input.trim(), model: $selectedModel } });
    input = '';
  }
</script>
```

**Changes to `ChatList.svelte`:**
```svelte
<script lang="ts">
  import { dispatch } from '../lib/actions';
  
  async function handleSubmit() {
    const trimmed = chatTitle.trim();
    if (trimmed) {
      dispatch({ type: 'createChat', payload: { title: trimmed } });
      dialogEl.close();
    }
  }
</script>
```

**Changes to `ws.ts` (WebSocket event handler):**
```typescript
// Instead of calling handleChatEvent directly, dispatch action
function handleEvent(message: WsEvent): void {
  const { event, data } = message;
  
  if (event.startsWith('chat') || event.startsWith('project')) {
    // Dispatch as action instead of direct call
    dispatch({ type: event as any, payload: data });
    return;
  }
  // ... other events
}
```

### Phase 4: Rewrite Tests with Action Overrides

**Remove old UI-based tests:**
- `frontend/src/tests/e2e/chat.test.ts`
- `frontend/src/tests/e2e/chat-messageflow.test.ts`
- `frontend/src/tests/e2e/chat-streaming.test.ts`
- `frontend/src/tests/e2e/projects.test.ts`
- `frontend/src/tests/e2e/scenarios.test.ts`

**Extend state-based tests:**
```typescript
// e2e/chat-state.test.ts
/**
 * Test cases covered:
 * - chat-create.md
 * - chat-send-message.md
 * - chat-streaming.md
 */

import { describe, it, expect, beforeAll, afterAll, beforeEach } from 'vitest';
import { dispatch, _testOverrideAction, _testClearOverrides } from '../../lib/actions';
import { chats, messages, isStreaming, resetAllStores } from '../../lib/chatStores';

describe('Chat state logic', () => {
  beforeEach(() => {
    resetAllStores();
    _testClearOverrides();
  });

  it('creates chat and updates state', async () => {
    // Override sendMessage to simulate daemon response
    _testOverrideAction('createChat', async (action) => {
      // Simulate successful chat creation
      chats.update(list => [...list, { id: 1, title: action.payload.title, ... }]);
      currentChatId.set(1);
    });

    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });

    expect(get(chats).length).toBe(1);
    expect(get(currentChatId)).toBe(1);
  });

  it('sends message and receives streaming response', async () => {
    // Setup: create chat first
    _testOverrideAction('createChat', async () => {
      chats.set([{ id: 1, title: 'Test', ... }]);
      currentChatId.set(1);
    });
    await dispatch({ type: 'createChat', payload: { title: 'Test' } });

    // Override sendMessage to simulate streaming
    _testOverrideAction('sendMessage', async (action) => {
      // Add user message
      messages.update(list => [...list, { id: 1, role: 'user', content: action.payload.content }]);
      isStreaming.set(true);
      
      // Simulate streaming chunks
      await dispatch({ type: 'chatStreamChunk', payload: { content: 'AI ' } });
      await dispatch({ type: 'chatStreamChunk', payload: { content: 'response' } });
      await dispatch({ type: 'chatStreamFinished' });
    });

    await dispatch({ type: 'sendMessage', payload: { content: 'Hello', model: 'test_model' } });

    expect(get(isStreaming)).toBe(false);
    const assistantMsg = get(messages).find(m => m.role === 'assistant');
    expect(assistantMsg.content).toBe('AI response');
  });
});
```

### Phase 5: Create UI Tests (Separate)

**Location:** `frontend/src/tests/ui/`

**Purpose:** Test UI rendering and action dispatch

```typescript
// ui/MessageInput.test.ts
/**
 * Test cases covered:
 * - chat-send-message.md (UI part)
 */

import { render, screen, fireEvent } from '@testing-library/svelte';
import MessageInput from '../../components/MessageInput.svelte';
import { selectedModel, availableModels } from '../../lib/chatStores';
import { _testOverrideAction, _testClearOverrides } from '../../lib/actions';
import type { ChatAction } from '../../lib/actions/types';

describe('MessageInput UI', () => {
  beforeEach(() => {
    _testClearOverrides();
  });

  it('renders send button when not streaming', () => {
    availableModels.set(['model1']);
    selectedModel.set('model1');
    
    const { getByText } = render(MessageInput);
    expect(getByText('Send')).toBeTruthy();
  });

  it('dispatches sendMessage action when send clicked', async () => {
    const dispatched: ChatAction[] = [];
    _testOverrideAction('sendMessage', async (action) => {
      dispatched.push(action);
    });
    
    availableModels.set(['model1']);
    selectedModel.set('model1');
    
    const { getByText, getByPlaceholderText } = render(MessageInput);
    await fireEvent.input(getByPlaceholderText('Type a message...'), {
      target: { value: 'Hello' },
    });
    await fireEvent.click(getByText('Send'));
    
    expect(dispatched).toContainEqual({
      type: 'sendMessage',
      payload: { content: 'Hello', model: 'model1' },
    });
  });
});
```

---

## File Changes Summary

### New Files

| File | Purpose |
|------|---------|
| `tests/cases/*.md` | Human-readable test cases |
| `frontend/src/lib/actions/types.ts` | Action type definitions |
| `frontend/src/lib/actions/dispatcher.ts` | ActionDispatcher with `_testOverrideAction` |
| `frontend/src/lib/actions/processors.ts` | Action processors (wrap chatWs) |
| `frontend/src/lib/actions/index.ts` | Public exports |
| `frontend/src/tests/ui/*.test.ts` | UI tests |

### Modified Files

| File | Changes |
|------|---------|
| `frontend/src/components/MessageInput.svelte` | Use `dispatch()` instead of direct calls |
| `frontend/src/components/ChatList.svelte` | Use `dispatch()` instead of direct calls |
| `frontend/src/components/ChatView.svelte` | Use `dispatch()` for edit/abort/pause/resume |
| `frontend/src/lib/ws.ts` | Dispatch actions for chat events instead of direct `handleChatEvent` call |
| `frontend/src/tests/e2e/chat-state.test.ts` | Add test case references, use action overrides |

### Removed Files

| File | Reason |
|------|--------|
| `frontend/src/tests/e2e/chat.test.ts` | Replaced by state-based + UI tests |
| `frontend/src/tests/e2e/chat-messageflow.test.ts` | Replaced by state-based tests |
| `frontend/src/tests/e2e/chat-streaming.test.ts` | Replaced by state-based tests |
| `frontend/src/tests/e2e/projects.test.ts` | Replaced by state-based tests |
| `frontend/src/tests/e2e/scenarios.test.ts` | Replaced by state-based tests |

---

## Migration Strategy

1. **Phase 1:** Create test case markdown files (non-breaking)
2. **Phase 2:** Add actions layer with dispatcher and processors (non-breaking)
3. **Phase 3:** Refactor components to use `dispatch()` (breaking, but internal)
4. **Phase 4:** Remove old tests, rewrite using action overrides
5. **Phase 5:** Add UI tests

---

## Benefits

1. **Action-based interception** — Tests override at dispatcher level, not method level
2. **Clear separation** — UI emits actions, dispatcher routes, processors handle
3. **Human-readable specs** — Test cases document expected behavior
4. **Flexible mocking** — Override handlers can dispatch follow-up actions
5. **Faster tests** — State tests don't render UI
6. **Better coverage** — Can test edge cases in processors

---

## Questions for Clarification

1. Should system actions (chatStreamChunk, etc.) be part of ChatAction union or separate?
2. Should processors be async or sync?
3. Should we keep `chatWs.ts` functions as-is and just add actions layer on top?
