# Frontend State-Based Testing Paradigm

## Status: COMPLETED

## Overview

Shift frontend E2E testing from UI-rendering-based approach to state-logic-based approach. Tests will verify state changes and daemon communication directly, without rendering components. UI correctness tests will be separate.

---

## Current State Analysis

### Current Testing Approach (Problematic)

Current E2E tests in `frontend/src/tests/e2e/`:
- Render full components (`ChatsTab.svelte`)
- Interact with DOM elements (buttons, textareas, selects)
- Check both state AND UI rendering
- Mix concerns: state logic verification + UI correctness

**Issues:**
1. Tests fail when UI changes even if state logic is correct
2. Hard to isolate state logic bugs from rendering bugs
3. Slow tests due to full component rendering
4. Flaky tests due to DOM timing issues

### Recent Changes (chat-visibility-enhancements.md)

Last implemented feature added:
- System prompt visibility (collapsible sections)
- AI thinking/reasoning content support
- MCP tool calls visualization

These changes likely broke existing tests because:
- New DOM elements added (collapsible sections, thinking content)
- Message structure changed (added `thinkingContent`, `mcpName` fields)
- Streaming behavior modified (thinking chunks handling)

---

## New Testing Paradigm

### Principle: Test State Logic, Not UI

**State-based E2E tests:**
- Still start `rhd_test frontend` to spawn real daemon
- Call state manipulation functions directly (no rendering)
- Verify store changes
- Verify actual WebSocket communication with daemon
- Use mock AI server (via control server) to control responses

**UI correctness tests (separate):**
- Render components with mocked state
- Trigger events manually
- Verify UI renders correctly
- These are unit/integration tests, not E2E

---

## Implementation Plan

### Phase 1: Expose State Manipulation Functions

**Goal:** Make state logic testable without UI

**Changes needed in `frontend/src/lib/chatWs.ts`:**

Currently, functions like `sendMessage()`, `createChat()` are tied to UI flow. Need to:

1. **Separate state mutation from UI triggers**
   - Extract pure state manipulation logic
   - Make functions callable without DOM interaction
   - Ensure all state changes go through testable functions

2. **Export internal state functions for testing**
   ```typescript
   // Example: expose state manipulation
   export function _test_createChat(title: string): Promise<WsResponse>
   export function _test_sendMessage(content: string, model: string): Promise<WsResponse>
   export function _test_handleChatEvent(event: string, data: unknown): void
   ```

3. **Add state reset function**
   ```typescript
   export function _test_resetState(): void
   ```

**Files to modify:**
- `frontend/src/lib/chatWs.ts` - expose testable functions
- `frontend/src/lib/chatStores.ts` - add reset capabilities
- `frontend/src/tests/testUtils.ts` - add state testing utilities

### Phase 2: Refactor Existing Tests to State-Based

**Current tests to refactor:**

1. **`chat.test.ts`** - "creates new chat with model pre-selected"
   - Current: Renders ChatsTab, clicks button, checks DOM
   - New: Call `createChat()` directly, verify `chats` store updated, verify `currentChatId` set

2. **`chat-messageflow.test.ts`** - "sends message and receives response"
   - Current: Renders UI, fills textarea, clicks send, checks DOM
   - New: Call `createChat()`, then `sendMessage()`, verify `messages` store, verify `isStreaming` state transitions

3. **`chat-streaming.test.ts`** - "renders streaming chunks in real-time"
   - Current: Renders UI, manually controls stream chunks, checks DOM elements
   - New: Call state functions, verify store updates as daemon sends real WebSocket events

**New test structure:**

Tests still start `rhd_test frontend` to spawn the real daemon. Tests call state manipulation functions directly (no UI rendering) and verify actual WebSocket communication with the daemon.

```typescript
// chat-state.test.ts
import { spawn, ChildProcess } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { get } from 'svelte/store';
import { _test_createChat, _test_sendMessage, _test_resetState } from '../lib/chatWs';
import { chats, messages, isStreaming, currentChatId } from '../lib/chatStores';
import { waitForWebSocket, setControlPort, setWsPort, configureMock } from '../testUtils';
import { setWsPort as setWsWsPort, connectWebSocket } from '../lib/ws';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

let rhdProcess: ChildProcess | null = null;

beforeAll(async () => {
  // Start rhd_test frontend (spawns real daemon)
  const workspaceRoot = resolve(__dirname, '../../../..');
  const rhdTestBin = resolve(workspaceRoot, 'target/debug/rhd_test');
  
  rhdProcess = spawn(rhdTestBin, ['frontend'], {
    cwd: workspaceRoot,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  
  let stdoutBuffer = '';
  const portsPromise = new Promise<{ controlPort: number; wsPort: number }>((resolve) => {
    rhdProcess!.stdout?.on('data', (data: Buffer) => {
      const text = data.toString();
      stdoutBuffer += text;
      
      const controlMatch = stdoutBuffer.match(/Control server started on port (\d+)/);
      const wsMatch = stdoutBuffer.match(/WebSocket server started on port (\d+)/);
      
      if (controlMatch && wsMatch) {
        resolve({
          controlPort: parseInt(controlMatch[1], 10),
          wsPort: parseInt(wsMatch[1], 10),
        });
      }
    });
  });
  
  // Wait for ports and connect WebSocket
  const { controlPort, wsPort } = await portsPromise;
  setControlPort(controlPort);
  setWsPort(wsPort);
  setWsWsPort(wsPort);
  connectWebSocket();
  await waitForWebSocket();
}, 30000);

afterAll(() => {
  if (rhdProcess) {
    rhdProcess.kill('SIGTERM');
    rhdProcess = null;
  }
});

describe('Chat state logic', () => {
  beforeEach(() => {
    _test_resetState();
  });

  it('creates chat via daemon and updates state', async () => {
    // Call state function - communicates with real daemon via WebSocket
    const response = await _test_createChat('Test Chat');
    
    // Verify response from daemon
    expect(response.success).toBe(true);
    
    // Verify state updated
    expect(get(chats).length).toBe(1);
    expect(get(currentChatId)).toBe(response.data.chatId);
  });

  it('sends message and receives streaming response from daemon', async () => {
    await _test_createChat('Test');
    
    // Configure mock AI response via control server
    await configureMock('AI response content');
    
    // Send message - communicates with daemon
    const sendPromise = _test_sendMessage('Hello', 'model1');
    
    // Verify streaming state set immediately
    expect(get(isStreaming)).toBe(true);
    
    // Wait for daemon to process and send response via WebSocket
    // State updates happen via handleChatEvent called by WebSocket handler
    await waitFor(() => {
      expect(get(messages).length).toBe(2); // user + assistant
      expect(get(isStreaming)).toBe(false);
    });
    
    // Verify message content from daemon
    const assistantMsg = get(messages).find(m => m.role === 'assistant');
    expect(assistantMsg.content).toBe('AI response content');
  });
});
```

### Phase 3: Fix Failing Tests

**Investigate current failures:**

Based on chat-visibility-enhancements changes, likely failures:

1. **Model selection test** - May fail if model auto-selection logic changed
2. **Message flow test** - May fail if message structure changed (thinkingContent, mcpName)
3. **Streaming test** - May fail if streaming state management changed

**Fix approach:**
1. Run tests to identify exact failures
2. Check if failures are due to:
   - State logic bugs (need code fix)
   - Test expectations outdated (need test update)
   - Missing state exposure (need Phase 1 work)

### Phase 4: Create UI Correctness Tests (Separate)

**New test category:** `frontend/src/tests/ui/`

**Purpose:** Verify UI renders correctly given specific state

**Approach:**
- Mock state stores with specific values
- Render components
- Trigger events manually
- Verify DOM output

**Example:**

```typescript
// ui/Message.test.ts
import { render } from '@testing-library/svelte';
import Message from '../../components/Message.svelte';
import { messages } from '../../lib/chatStores';

describe('Message UI', () => {
  it('renders thinking content when present', () => {
    // Mock state
    messages.set([{
      id: 1,
      role: 'assistant',
      content: 'Response',
      thinkingContent: 'Thinking process...'
    }]);

    const { getByText } = render(Message, { props: { message: get(messages)[0] } });
    expect(getByText('Thinking')).toBeTruthy();
  });

  it('collapses thinking by default', () => {
    // Verify collapsed state
  });
});
```

### Phase 5: Update Test Infrastructure

**Changes to `frontend/src/tests/testUtils.ts`:**

Add utilities for state-based testing:

```typescript
// Reset all stores to initial state
export function resetAllStores(): void

// Wait for specific store value
export function waitForStoreValue<T>(store: Readable<T>, expected: T): Promise<void>
```

**Changes to `frontend/vitest.config.e2e.ts`:**

- Separate state-based tests from UI tests
- Different test patterns for each category

---

## Code Changes Required

### 1. `frontend/src/lib/chatWs.ts`

**Add test-only exports:**

```typescript
// Test-only exports (prefixed with _test_)
export function _test_resetState(): void {
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
  pendingToolCalls.set([]);
}

// Re-export existing functions for clarity
export {
  createChat as _test_createChat,
  sendMessage as _test_sendMessage,
  handleChatEvent as _test_handleChatEvent,
  // ... etc
}
```

### 2. `frontend/src/lib/chatStores.ts`

**Add reset function:**

```typescript
export function resetAllStores(): void {
  chats.set([]);
  currentChatId.set(null);
  messages.set([]);
  // ... reset all stores
}
```

### 3. `frontend/src/tests/testUtils.ts`

**Add state testing utilities:**

```typescript
export async function waitForStoreValue<T>(
  store: Readable<T>,
  predicate: (value: T) => boolean,
  timeout = 5000
): Promise<void> {
  // Wait for store to match predicate
}
```

---

## Test Migration Examples

### Example 1: Chat Creation Test

**Before (UI-based):**
```typescript
render(ChatsTab);
const newChatButton = screen.getByText('+ New Chat');
await fireEvent.click(newChatButton);
await waitFor(() => {
  const modelSelect = screen.getByLabelText('Model:');
  expect(modelSelect.options.length).toBeGreaterThan(0);
});
```

**After (State-based):**
```typescript
const response = await _test_createChat('Test Chat');
expect(response.success).toBe(true);
expect(get(chats).length).toBe(1);
expect(get(currentChatId)).toBe(response.data.chatId);
```

### Example 2: Message Flow Test

**Before (UI-based):**
```typescript
render(ChatsTab);
const textarea = screen.getByPlaceholderText('Type a message...');
await fireEvent.input(textarea, { target: { value: 'Hello' } });
await fireEvent.click(screen.getByText('Send'));
await waitFor(() => {
  expect(get(messages).length).toBe(2);
});
```

**After (State-based):**
```typescript
await _test_createChat('Test');
await configureMock('AI response');
await _test_sendMessage('Hello', 'model1');

// Wait for real daemon response via WebSocket
await waitFor(() => {
  expect(get(messages).length).toBe(2);
  expect(get(isStreaming)).toBe(false);
});
```

---

## Benefits of New Paradigm

1. **Faster tests** - No component rendering overhead
2. **More stable** - No DOM timing issues
3. **Clearer failures** - State logic bugs isolated from UI bugs
4. **Easier maintenance** - State tests don't break when UI changes
5. **Better coverage** - Can test edge cases in state logic easily
6. **Real integration** - Still tests actual daemon communication

---

## Migration Strategy

1. **Phase 1:** Expose state functions (non-breaking)
2. **Phase 2:** Fix current failing tests (urgent)
3. **Phase 3:** Gradually migrate existing tests to state-based
4. **Phase 4:** Add UI correctness tests separately
5. **Phase 5:** Update documentation and memory

---

## Next Steps

1. Investigate current test failures (run tests, analyze errors)
2. Implement Phase 1 (expose state functions)
3. Fix failing tests using new approach
4. Refactor remaining tests
5. Document new paradigm in `memory/frontend-e2e.md`

---

## Questions for Clarification

1. Should we keep old UI-based tests during transition, or replace immediately?
2. Do we need separate test commands for state-based vs UI tests?
3. Should state manipulation functions be in separate file (e.g., `chatState.ts`)?
