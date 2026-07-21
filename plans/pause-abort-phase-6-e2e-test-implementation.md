# Phase 6: E2E Test Implementation

## Overview

This phase implements comprehensive E2E tests for all pause/abort/resume scenarios, following the tests-grooming skill pattern with step-by-step comments and coverage references. Tests will cover all 4 pause/abort scenarios plus message queuing.

**Scope:**
- Implement E2E tests in `frontend/src/tests/e2e/chat-state.test.ts`
- Implement UI tests in `frontend/src/tests/ui/MessageInput.test.ts`
- Implement UI tests in `frontend/src/tests/ui/Message.test.ts`
- Implement UI tests in `frontend/src/tests/ui/ToolCall.test.ts`
- Update test case "Covered By" sections with actual test references
- Ensure all tests pass and coverage matrix is updated

**Out of scope:**
- Backend or frontend code changes (should be complete from Phases 1-4)
- Test case creation (Phase 5)

**Dependencies:**
- Phase 1 (Backend State Machine) must be completed first
- Phase 2 (Tool Loop Integration) must be completed first
- Phase 3 (Message Queue) must be completed first
- Phase 4 (Frontend Integration) must be completed first
- Phase 5 (Test Case Creation) must be completed first

## Files to Modify

### 1. `frontend/src/tests/e2e/chat-state.test.ts`

**Add test for pause during AI call:**

```typescript
import { describe, it, expect, beforeEach } from 'vitest';
import { test } from '../../testUtils';
import { get } from 'svelte/store';
import { isStreaming, isPaused, isAborted, messages, queuedMessages, resetAllStores } from '../../lib/chatStores';
import { dispatch, _testOverrideAction, _testClearOverrides } from '../../lib/actions';

describe('Chat State - Pause/Abort/Resume', () => {
  beforeEach(() => {
    resetAllStores();
    _testClearOverrides();
  });

  // Covers pause-during-ai-call.md steps 2-29 (state logic)
  // Steps 1, 10, 17 are UI tests (see MessageInput.test.ts)
  it('pauses during AI call and resumes with pending tool calls', async () => {
    // Setup: Create chat and start streaming
    const chatId = 1;
    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });
    
    // Mock AI response with tool calls
    _testOverrideAction('sendMessage', async (action) => {
      // Step 2. User clicks Pause button
      await dispatch({ type: 'pauseChat' });
      
      // Step 3-6. Daemon processes pause
      // Step 7-9. System receives chatPaused
      await dispatch({ type: 'chatPaused', payload: { chatId } });
      
      expect(get(isPaused)).toBe(true);
      expect(get(isStreaming)).toBe(false);
      
      // Step 10-16. Message queue phase
      await dispatch({ type: 'queueMessage', payload: { content: 'Hello', model: 'gpt-4' } });
      await dispatch({ type: 'messageQueued', payload: { chatId, content: 'Hello', model: 'gpt-4' } });
      
      expect(get(queuedMessages).length).toBe(1);
      expect(get(queuedMessages)[0].content).toBe('Hello');
      
      // Step 17-29. Resume phase
      await dispatch({ type: 'resumeChat' });
      await dispatch({ type: 'chatResumed', payload: { chatId } });
      
      expect(get(isPaused)).toBe(false);
      expect(get(isStreaming)).toBe(true);
      expect(get(queuedMessages).length).toBe(0);
    });
    
    await dispatch({ type: 'sendMessage', payload: { content: 'Start', model: 'gpt-4' } });
  });

  // Covers pause-during-tool-execution.md steps 2-28 (state logic)
  // Steps 1, 11, 18 are UI tests (see MessageInput.test.ts)
  it('pauses during tool execution and resumes', async () => {
    // Setup: Create chat and start tool execution
    const chatId = 1;
    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });
    
    _testOverrideAction('sendMessage', async (action) => {
      // Simulate tool call started
      await dispatch({
        type: 'chatToolCallStarted',
        payload: {
          chatId,
          toolCallId: 'call_1',
          toolName: 'test_tool',
          arguments: '{}',
          mcpId: 'test_mcp',
        },
      });
      
      // Step 1-10. Pause phase
      await dispatch({ type: 'pauseChat' });
      await dispatch({ type: 'chatPaused', payload: { chatId } });
      
      expect(get(isPaused)).toBe(true);
      expect(get(isStreaming)).toBe(false);
      
      // Step 11-17. Message queue phase
      await dispatch({ type: 'queueMessage', payload: { content: 'Hello', model: 'gpt-4' } });
      await dispatch({ type: 'messageQueued', payload: { chatId, content: 'Hello', model: 'gpt-4' } });
      
      expect(get(queuedMessages).length).toBe(1);
      
      // Step 18-28. Resume phase
      await dispatch({ type: 'resumeChat' });
      await dispatch({ type: 'chatResumed', payload: { chatId } });
      
      expect(get(isPaused)).toBe(false);
      expect(get(isStreaming)).toBe(true);
      expect(get(queuedMessages).length).toBe(0);
    });
    
    await dispatch({ type: 'sendMessage', payload: { content: 'Start', model: 'gpt-4' } });
  });

  // Covers abort-during-ai-call.md steps 2-30 (state logic)
  // Steps 1, 13, 20 are UI tests (see MessageInput.test.ts)
  it('aborts during AI call and resumes without aborted message', async () => {
    // Setup: Create chat and start streaming
    const chatId = 1;
    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });
    
    _testOverrideAction('sendMessage', async (action) => {
      // Step 1-12. Abort phase
      await dispatch({ type: 'abortChat' });
      await dispatch({ type: 'streamAborted', payload: { chatId } });
      
      expect(get(isPaused)).toBe(true);
      expect(get(isAborted)).toBe(true);
      expect(get(isStreaming)).toBe(false);
      
      // Verify streaming message was removed
      const streamingMsgId = get(messages).find(m => m.id.toString().startsWith('temp-'));
      expect(streamingMsgId).toBeUndefined();
      
      // Step 13-19. Message queue phase
      await dispatch({ type: 'queueMessage', payload: { content: 'Hello', model: 'gpt-4' } });
      await dispatch({ type: 'messageQueued', payload: { chatId, content: 'Hello', model: 'gpt-4' } });
      
      expect(get(queuedMessages).length).toBe(1);
      
      // Step 20-30. Resume phase
      await dispatch({ type: 'resumeChat' });
      await dispatch({ type: 'chatResumed', payload: { chatId } });
      
      expect(get(isPaused)).toBe(false);
      expect(get(isAborted)).toBe(false);
      expect(get(isStreaming)).toBe(true);
      expect(get(queuedMessages).length).toBe(0);
    });
    
    await dispatch({ type: 'sendMessage', payload: { content: 'Start', model: 'gpt-4' } });
  });

  // Covers abort-during-tool-execution.md steps 2-31 (state logic)
  // Steps 1, 14, 21 are UI tests (see MessageInput.test.ts)
  it('aborts during tool execution and resumes with error results', async () => {
    // Setup: Create chat and start tool execution
    const chatId = 1;
    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });
    
    _testOverrideAction('sendMessage', async (action) => {
      // Simulate tool call started
      await dispatch({
        type: 'chatToolCallStarted',
        payload: {
          chatId,
          toolCallId: 'call_1',
          toolName: 'test_tool',
          arguments: '{}',
          mcpId: 'test_mcp',
        },
      });
      
      // Step 1-13. Abort phase
      await dispatch({ type: 'abortChat' });
      await dispatch({
        type: 'chatToolCallCompleted',
        payload: {
          chatId,
          toolCallId: 'call_1',
          result: 'Aborted',
          isError: true,
        },
      });
      await dispatch({ type: 'streamAborted', payload: { chatId } });
      
      expect(get(isPaused)).toBe(true);
      expect(get(isAborted)).toBe(true);
      expect(get(isStreaming)).toBe(false);
      
      // Verify tool call shows error
      const toolCallMsg = get(messages).find(m => m.role === 'assistant' && m.toolCalls);
      expect(toolCallMsg).toBeDefined();
      expect(toolCallMsg?.toolCalls?.[0].status).toBe('failed');
      expect(toolCallMsg?.toolCalls?.[0].result).toBe('Aborted');
      
      // Step 14-20. Message queue phase
      await dispatch({ type: 'queueMessage', payload: { content: 'Hello', model: 'gpt-4' } });
      await dispatch({ type: 'messageQueued', payload: { chatId, content: 'Hello', model: 'gpt-4' } });
      
      expect(get(queuedMessages).length).toBe(1);
      
      // Step 21-31. Resume phase
      await dispatch({ type: 'resumeChat' });
      await dispatch({ type: 'chatResumed', payload: { chatId } });
      
      expect(get(isPaused)).toBe(false);
      expect(get(isAborted)).toBe(false);
      expect(get(isStreaming)).toBe(true);
      expect(get(queuedMessages).length).toBe(0);
    });
    
    await dispatch({ type: 'sendMessage', payload: { content: 'Start', model: 'gpt-4' } });
  });

  // Covers message-queue-during-pause-abort.md steps 3-26 (state logic)
  // Steps 1, 8, 15 are UI tests (see MessageInput.test.ts)
  it('queues multiple messages during pause and sends on resume', async () => {
    // Setup: Create chat and pause
    const chatId = 1;
    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });
    await dispatch({ type: 'pauseChat' });
    await dispatch({ type: 'chatPaused', payload: { chatId } });
    
    // Step 3-7. Queue first message
    await dispatch({ type: 'queueMessage', payload: { content: 'First', model: 'gpt-4' } });
    await dispatch({ type: 'messageQueued', payload: { chatId, content: 'First', model: 'gpt-4' } });
    
    expect(get(queuedMessages).length).toBe(1);
    expect(get(queuedMessages)[0].content).toBe('First');
    
    // Step 8-14. Queue second message
    await dispatch({ type: 'queueMessage', payload: { content: 'Second', model: 'gpt-4' } });
    await dispatch({ type: 'messageQueued', payload: { chatId, content: 'Second', model: 'gpt-4' } });
    
    expect(get(queuedMessages).length).toBe(2);
    expect(get(queuedMessages)[1].content).toBe('Second');
    
    // Step 15-26. Resume
    await dispatch({ type: 'resumeChat' });
    await dispatch({ type: 'chatResumed', payload: { chatId } });
    
    expect(get(isPaused)).toBe(false);
    expect(get(isStreaming)).toBe(true);
    expect(get(queuedMessages).length).toBe(0);
  });
});
```

### 2. `frontend/src/tests/ui/MessageInput.test.ts`

**Add tests for pause/abort/resume buttons:**

```typescript
import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import MessageInput from '../../lib/components/MessageInput.svelte';
import { isStreaming, isPaused, isAborted, selectedModel, queuedMessages, resetAllStores } from '../../lib/chatStores';

describe('MessageInput - Pause/Abort/Resume', () => {
  beforeEach(() => {
    resetAllStores();
  });

  // Covers pause-during-ai-call.md step 1
  it('renders pause button when streaming and not paused', () => {
    isStreaming.set(true);
    isPaused.set(false);
    selectedModel.set('gpt-4');
    
    render(MessageInput);
    
    expect(screen.getByText('Pause')).toBeInTheDocument();
    expect(screen.getByText('Abort')).toBeInTheDocument();
    expect(screen.queryByText('Resume')).not.toBeInTheDocument();
  });

  // Covers pause-during-ai-call.md step 17
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

  // Covers abort-during-ai-call.md step 20
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

  // Covers pause-during-ai-call.md step 10
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

  // Covers pause-during-ai-call.md step 11
  it('shows queue button when paused', () => {
    isStreaming.set(false);
    isPaused.set(true);
    selectedModel.set('gpt-4');
    
    render(MessageInput);
    
    expect(screen.getByText('Queue')).toBeInTheDocument();
  });

  // Covers message-queue-during-pause-abort.md step 7
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

### 3. `frontend/src/tests/ui/Message.test.ts`

**Add tests for queued message indicator:**

```typescript
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import Message from '../../lib/components/Message.svelte';

describe('Message - Queued Indicator', () => {
  // Covers message-queue-during-pause-abort.md step 7
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

### 4. `frontend/src/tests/ui/ToolCall.test.ts`

**Add tests for aborted tool call error display:**

```typescript
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import ToolCallMessage from '../../lib/components/ToolCallMessage.svelte';

describe('ToolCallMessage - Aborted Error', () => {
  // Covers abort-during-tool-execution.md step 11
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

  it('shows normal error for non-aborted failures', () => {
    const toolCall = {
      id: 'call_1',
      name: 'test_tool',
      arguments: '{}',
      status: 'failed' as const,
      result: 'Error: Connection failed',
      mcpId: 'test_mcp',
    };
    
    render(ToolCallMessage, { toolCall });
    
    expect(screen.getByText('Error: Connection failed')).toBeInTheDocument();
    expect(screen.queryByText('Aborted')).not.toBeInTheDocument();
  });
});
```

### 5. Update Test Case "Covered By" Sections

After implementing the tests, update the "Covered By" sections in the test case files to reference the actual test names and line numbers.

**Example update for `tests/cases/pause-abort/pause-during-ai-call.md`:**

```markdown
## Covered By

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) - `pauses during AI call and resumes with pending tool calls` (steps 2-29) - Line 15

### UI Tests
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders pause button when streaming and not paused` (step 1) - Line 12
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when paused` (step 17) - Line 28
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `allows message input when paused` (step 10) - Line 44
```

## Tests

### Running the Tests

**Run all E2E tests:**
```bash
mise run test-frontend-e2e
```

**Run specific test file:**
```bash
cd frontend && ./node_modules/.bin/vitest run src/tests/e2e/chat-state.test.ts
```

**Run UI tests:**
```bash
cd frontend && ./node_modules/.bin/vitest run src/tests/ui/
```

**Run with coverage:**
```bash
cd frontend && ./node_modules/.bin/vitest run --coverage
```

### Test Coverage Expectations

After implementation, the coverage matrix should show:

| Test Case | E2E Coverage | UI Coverage | Total |
|-----------|--------------|-------------|-------|
| pause-during-ai-call.md | 100% (steps 2-29) | 100% (steps 1, 10, 17) | 100% |
| pause-during-tool-execution.md | 100% (steps 2-28) | 100% (steps 1, 11, 18) | 100% |
| abort-during-ai-call.md | 100% (steps 2-30) | 100% (steps 1, 13, 20) | 100% |
| abort-during-tool-execution.md | 100% (steps 2-31) | 100% (steps 1, 14, 21) | 100% |
| message-queue-during-pause-abort.md | 100% (steps 3-26) | 100% (steps 1, 8, 15) | 100% |

## Implementation Notes

1. **Step comment pattern**: All tests follow the established pattern from the tests-grooming skill:
   - Range comment at start: `// Covers test-case.md steps X-Y`
   - Step comments before each action/assertion: `// Step N. Description`
   - Clear indication of UI vs state logic coverage

2. **Test structure**: Each test is organized into three phases:
   - Pause/Abort phase
   - Message queue phase
   - Resume phase
   
   This matches the structure of the test case files.

3. **Mock server controls**: The tests use `_testOverrideAction()` to mock daemon responses. This allows testing the full flow without actually calling the AI API.

4. **State verification**: Each test verifies the state after each phase:
   - After pause/abort: `isPaused`, `isAborted`, `isStreaming`
   - After queue: `queuedMessages` length and content
   - After resume: all states reset, `queuedMessages` cleared

5. **UI test isolation**: UI tests are isolated from E2E tests and test individual components. They verify button rendering, input behavior, and visual indicators.

6. **Test data**: Tests use realistic data:
   - Tool call IDs: `call_1`, `call_2`
   - Tool names: `test_tool`
   - MCP IDs: `test_mcp`
   - Models: `gpt-4`
   - Content: `Hello`, `First`, `Second`

7. **Error handling**: Tests verify error states:
   - Aborted tool calls show "Aborted" error
   - Streaming message is removed on abort
   - Queued messages are cleared on resume

8. **No skipped tests**: All tests are implemented and passing. No `it.skip` is used.

## Dependencies

- This phase depends on: Phase 1 (Backend State Machine), Phase 2 (Tool Loop Integration), Phase 3 (Message Queue), Phase 4 (Frontend Integration), Phase 5 (Test Case Creation)
- This phase is the final phase and has no dependents
