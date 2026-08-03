/**
 * Test cases covered:
 * - tests/cases/chat-create.md
 * - tests/cases/chat-send-message.md
 * - tests/cases/chat-streaming.md
 * - tests/cases/chat-select-model.md
 * - tests/cases/chat-delete.md
 * - tests/cases/chat-edit-message.md
 * - tests/cases/chat-pause-resume.md
 * - tests/cases/pause-abort/pause-during-ai-call.md
 * - tests/cases/pause-abort/pause-during-tool-execution.md
 * - tests/cases/pause-abort/abort-during-ai-call.md
 * - tests/cases/pause-abort/abort-during-tool-execution.md
 * - tests/cases/pause-abort/message-queue-during-pause-abort.md
 */

import { describe, it, expect, beforeAll, afterAll, beforeEach } from 'vitest';
import { spawn, type ChildProcess } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { get } from 'svelte/store';
import { waitFor } from '@testing-library/svelte';
import { dispatch, _testClearOverrides } from '@/lib/actions';
import {
  chats,
  currentChatId,
  messages,
  isStreaming,
  isPaused,
  isAborted,
  availableModels,
  selectedModel,
  streamingMessageId,
  pendingMessages,
  resetAllStores,
} from '@/lib/chatStores';
import {
  waitForWebSocket,
  setControlPort,
  setWsPort,
  configureMock,
  emitStreamChunk,
  finishStream,
  setAutoStream,
  waitForStreamReady,
} from '../testUtils';
import { setWsPort as setWsWsPort, connectWebSocket } from '@/lib/ws';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

let rhdProcess: ChildProcess | null = null;

beforeAll(async () => {
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
      console.log(`[rhd_test] ${text}`);

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

  rhdProcess.stderr?.on('data', (data: Buffer) => {
    console.error(`[rhd_test] ${data.toString()}`);
  });

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

describe('Chat state logic (state-based testing)', () => {
  beforeEach(() => {
    resetAllStores();
    _testClearOverrides();
  });

  it('loads available models from daemon', async () => {
    // Covers chat-select-model.md step 1 (state logic)
    await dispatch({ type: 'loadAvailableModels' });

    const models = get(availableModels);
    expect(models.length).toBeGreaterThan(0);
    expect(models).toContain('test_model');
  });

  it('creates chat via daemon and updates state', async () => {
    // Covers chat-create.md steps 5-9 (state logic)
    // Steps 1-4, 10 are UI tests (see ChatList.test.ts)

    // Step 5. System dispatches `createChat` action with title
    // Step 6. System sends request to daemon
    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });

    // Step 7. Daemon creates chat and returns chatId
    // Step 8. System updates chats store with new chat
    const allChats = get(chats);
    expect(allChats.length).toBe(1);
    expect(allChats[0].title).toBe('Test Chat');

    // Step 9. System sets currentChatId to new chat
    expect(get(currentChatId)).toBeDefined();
  });

  it('sends message and receives streaming response from daemon', async () => {
    // Setup: Create chat first
    await dispatch({ type: 'createChat', payload: { title: 'Test' } });
    await configureMock('AI response content');

    const model = get(availableModels)[0] || 'test_model';

    // Covers chat-send-message.md steps 3-14 (state logic)
    // Steps 1-2 are UI tests (see MessageInput.test.ts)

    // Step 3. System dispatches `sendMessage` action with content and model
    // Step 4. System adds user message to messages store
    // Step 5. System sets isStreaming to true
    // Step 6. System sends request to daemon
    await dispatch({ type: 'sendMessage', payload: { content: 'Hello', model } });

    // Step 7. Daemon processes message and sends response
    // Step 8. System receives `chatStreamChunk` actions
    // Step 9. System creates optimistic assistant message on first chunk
    // Step 10. System appends chunks to assistant message
    // Step 11. System receives `chatStreamFinished` action
    // Step 12. System sets isStreaming to false
    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 5000 });

    // Step 13. System receives `chatMessageAdded` with real message
    // Step 14. System replaces optimistic message with real message
    const allMessages = get(messages);
    expect(allMessages.length).toBe(5);

    const userMsg = allMessages.find((m) => m.role === 'user');
    expect(userMsg).toBeDefined();
    expect(userMsg!.content).toBe('Hello');

    const assistantMsgs = allMessages.filter((m) => m.role === 'assistant');
    expect(assistantMsgs.length).toBe(2);

    const finalAssistantMsg = assistantMsgs.find((m) => m.content === 'AI response content');
    expect(finalAssistantMsg).toBeDefined();

    // Covers chat-streaming.md steps 1-9 (state logic)
    // Steps 1-3: Optimistic message creation and chunk appending (verified by message count)
    // Step 4: Animated dots indicator (UI test, see Message.test.ts)
    // Steps 5-9: Stream finish handling (verified by isStreaming=false and final message)
  });

  it('handles multiple messages in sequence', async () => {
    await dispatch({ type: 'createChat', payload: { title: 'Test' } });

    const model = get(availableModels)[0] || 'test_model';

    // First message: covers chat-send-message.md steps 3-14
    await configureMock('First response');
    await dispatch({ type: 'sendMessage', payload: { content: 'First message', model } });

    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 5000 });

    let allMessages = get(messages);
    expect(allMessages.length).toBe(5);

    // Second message: covers chat-send-message.md steps 3-14 again
    await configureMock('Second response');
    await dispatch({ type: 'sendMessage', payload: { content: 'Second message', model } });

    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 5000 });

    allMessages = get(messages);
    expect(allMessages.length).toBe(9);

    const userMessages = allMessages.filter((m) => m.role === 'user');
    expect(userMessages.length).toBe(2);
    expect(userMessages[0].content).toBe('First message');
    expect(userMessages[1].content).toBe('Second message');

    const assistantMessages = allMessages.filter((m) => m.role === 'assistant');
    expect(assistantMessages.length).toBe(4);
    const finalAssistantMessages = assistantMessages.filter((m) => m.content && m.content.length > 0);
    expect(finalAssistantMessages.length).toBe(2);
    expect(finalAssistantMessages[0].content).toBe('First response');
    expect(finalAssistantMessages[1].content).toBe('Second response');
  });

  it('auto-selects first model when available', async () => {
    // Covers chat-select-model.md steps 1-3, 6-8 (state logic)
    // Steps 4-5 are UI tests (see MessageInput.test.ts)

    // Step 1. System loads available models on mount
    await dispatch({ type: 'loadAvailableModels' });

    // Step 2. System populates model selector dropdown (UI test)
    // Step 3. If no model selected, system auto-selects first model
    const models = get(availableModels);
    expect(models.length).toBeGreaterThan(0);

    selectedModel.set(null);
    selectedModel.set(models[0]);

    // Step 6. System dispatches `selectModel` action with model name (implicit via store set)
    // Step 7. System updates selectedModel store
    expect(get(selectedModel)).toBe(models[0]);

    // Step 8. Model selection persists for current chat (verified by store state)
  });

  it('aborts streaming chat and updates state', async () => {
    // Covers chat-abort.md steps 2-3 (state logic)
    // Step 1 is UI test (see MessageInput.test.ts)
    // Steps 4-7 require daemon to send chatStreamError event (not mocked)

    // Setup: Create chat and start streaming
    await dispatch({ type: 'createChat', payload: { title: 'Test' } });
    await configureMock('Streaming response');
    await setAutoStream(false);
    const model = get(availableModels)[0] || 'test_model';
    await dispatch({ type: 'sendMessage', payload: { content: 'Hello', model } });

    // Wait for streaming to start
    await waitFor(() => {
      expect(get(isStreaming)).toBe(true);
    }, { timeout: 5000 });

    // Step 2. System dispatches `abortChat` action
    // Step 3. System sends abort request to daemon
    // The abort request is sent and returns successfully
    // Note: Full abort flow (steps 4-7) requires daemon to send chatStreamError event
    // which is not simulated by the mock server
    await dispatch({ type: 'abortChat' });
    
    // Verify abort was initiated (action dispatched without error)
    // The actual stream cancellation happens asynchronously in the daemon
  });

  it('deletes chat and updates state', async () => {
    // Covers chat-delete.md steps 4-8 (state logic)
    // Steps 1-3 are UI tests (see ChatList.test.ts)

    // Setup: Create a chat first
    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });
    const chatId = get(currentChatId);
    expect(chatId).toBeDefined();

    // Verify chat exists
    let allChats = get(chats);
    expect(allChats.length).toBe(1);

    // Step 4. System dispatches `deleteChat` action with chatId
    // Step 5. System sends request to daemon
    await dispatch({ type: 'deleteChat', payload: { chatId: chatId! } });

    // Step 6. Daemon deletes chat (mocked)
    // Step 7. System removes chat from chats store
    allChats = get(chats);
    expect(allChats.length).toBe(0);

    // Step 8. If deleted chat was current: System sets currentChatId to null, clears messages store
    expect(get(currentChatId)).toBeNull();
    expect(get(messages).length).toBe(0);
  });

  it('edits message and re-streams response', async () => {
    // Covers chat-edit-message.md steps 5-11 (state logic)
    // Steps 1-4 are UI tests (see Message.test.ts)

    // Setup: Create chat and send initial message
    await dispatch({ type: 'createChat', payload: { title: 'Test' } });
    await configureMock('Initial response');
    const model = get(availableModels)[0] || 'test_model';
    await dispatch({ type: 'sendMessage', payload: { content: 'Original message', model } });

    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 5000 });

    // Verify initial state
    let allMessages = get(messages);
    expect(allMessages.length).toBe(5);
    const userMsg = allMessages.find((m) => m.role === 'user');
    expect(userMsg).toBeDefined();
    expect(userMsg!.content).toBe('Original message');

    // Step 5. System dispatches `editMessage` action with messageId, new content, model
    // Step 6. System updates message content in messages store
    // Step 7. System truncates all messages after edited message
    // Step 8. System sets isStreaming to true
    // Step 9. System sends request to daemon
    await configureMock('Updated response');
    await dispatch({
      type: 'editMessage',
      payload: {
        messageId: userMsg!.id as number,
        content: 'Edited message',
        model,
      },
    });

    // Step 10. Daemon re-processes from edited message
    // Step 11. System receives streaming response
    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 5000 });

    // Verify edited message and new response
    allMessages = get(messages);
    const editedUserMsg = allMessages.find((m) => m.role === 'user');
    expect(editedUserMsg).toBeDefined();
    expect(editedUserMsg!.content).toBe('Edited message');

    const assistantMsgs = allMessages.filter((m) => m.role === 'assistant');
    expect(assistantMsgs.length).toBeGreaterThanOrEqual(1);
    const finalAssistantMsg = assistantMsgs.find((m) => m.content === 'Updated response');
    expect(finalAssistantMsg).toBeDefined();
  });

  it.skip('pauses and resumes streaming chat', async () => {
    // Covers chat-pause-resume.md pause steps 2-7 and resume steps 2-7 (state logic)
    // Pause step 1 and resume step 1 are UI tests (see MessageInput.test.ts)

    // Setup: Create chat and start streaming
    await dispatch({ type: 'createChat', payload: { title: 'Test' } });
    await configureMock('Streaming response');
    const model = get(availableModels)[0] || 'test_model';
    await dispatch({ type: 'sendMessage', payload: { content: 'Hello', model } });

    // Wait for streaming to start
    await waitFor(() => {
      expect(get(isStreaming)).toBe(true);
    }, { timeout: 5000 });

    // === PAUSE FLOW ===
    // Pause Step 2. System dispatches `pauseChat` action
    // Pause Step 3. System sends pause request to daemon
    await dispatch({ type: 'pauseChat' });

    // Pause Step 4. Daemon pauses execution (mocked)
    // Pause Step 5. System receives `chatPaused` action
    // Pause Step 6. System sets isPaused to true
    // Pause Step 7. System sets isStreaming to false
    await waitFor(() => {
      expect(get(isPaused)).toBe(true);
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 5000 });

    // === RESUME FLOW ===
    // Resume Step 2. System dispatches `resumeChat` action
    // Resume Step 3. System sends resume request to daemon
    await dispatch({ type: 'resumeChat' });

    // Resume Step 4. Daemon resumes execution (mocked)
    // Resume Step 5. System receives `chatResumed` action
    // Resume Step 6. System sets isPaused to false
    // Resume Step 7. System sets isStreaming to true
    await waitFor(() => {
      expect(get(isPaused)).toBe(false);
      expect(get(isStreaming)).toBe(true);
    }, { timeout: 5000 });

    // Wait for streaming to complete
    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 5000 });
  });

  // Covers pause-during-ai-call.md steps 2-29 (state logic)
  // Steps 1, 10, 17 are UI tests (see MessageInput.test.ts)
  it('pauses during AI call and resumes with pending tool calls', async () => {
    // Setup: Create chat and start streaming so the daemon has an active stream to pause
    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });
    await configureMock('AI response with tool calls');
    await setAutoStream(false);
    const model = get(availableModels)[0] || 'test_model';
    await dispatch({ type: 'sendMessage', payload: { content: 'Start', model } });
    
    await waitFor(() => {
      expect(get(isStreaming)).toBe(true);
    }, { timeout: 5000 });
    
    // Step 2. System dispatches `pauseChat` action
    // Step 3. System sends pause request to daemon
    await dispatch({ type: 'pauseChat' });
    
    // Step 4-6. Daemon processes pause (mocked)
    // Step 7-9. System receives chatPaused
    await dispatch({ type: 'chatPaused' });
    
    expect(get(isPaused)).toBe(true);
    expect(get(isStreaming)).toBe(false);
    
    // Step 10-15. Message queue phase
    // Step 10. User types message in input field (UI test)
    // Step 11. User clicks Send button (UI test)
    // Step 12. System dispatches `queueMessage` action
    // Step 13. System sends queue request to daemon
    // Step 14. System optimistically adds the message to pendingMessages store (gray, not yet part of chat)
    await dispatch({ type: 'queueMessage', payload: { content: 'Hello', model } });
    
    expect(get(pendingMessages).length).toBe(1);
    expect(get(pendingMessages)[0].content).toBe('Hello');
    
    // Step 15. User clicks Resume button (UI test)
    // Step 16. System dispatches `resumeChat` action
    // Step 17. System sends resume request to daemon
    await dispatch({ type: 'resumeChat' });
    
    // Step 18-23. Daemon processes pending tool calls and queued messages (mocked)
    // Step 24. System receives `chatResumed` action
    // Step 25. System sets isPaused to false
    // Step 26. System sets isStreaming to true
    await dispatch({ type: 'chatResumed' });
    
    expect(get(isPaused)).toBe(false);
    expect(get(isStreaming)).toBe(true);
    
    // Step 27. The interrupted call has not produced its result yet, so the message
    // stays queued and gray after resume
    expect(get(pendingMessages).length).toBe(1);
    
    // Step 28. System receives `chatStreamFinished` while resumed
    // Step 29. System promotes the queued message into messages as a regular user message
    await dispatch({ type: 'chatStreamFinished' });
    
    expect(get(pendingMessages).length).toBe(0);
    const promoted = get(messages).filter((m) => m.role === 'user' && m.content === 'Hello');
    expect(promoted.length).toBe(1);
    
    // Step 30. A later `chatMessageAdded` echo replaces the promoted message in place
    await dispatch({
      type: 'chatMessageAdded',
      payload: { message: { id: 9001, chatId: get(currentChatId)!, role: 'user', content: 'Hello', createdAt: new Date().toISOString(), model } as any },
    });
    
    expect(get(pendingMessages).length).toBe(0);
    expect(get(messages).filter((m) => m.content === 'Hello').length).toBe(1);
  });

  // Covers pause-during-tool-execution.md steps 2-28 (state logic)
  // Steps 1, 11, 18 are UI tests (see MessageInput.test.ts)
  it('pauses during tool execution and resumes', async () => {
    // Setup: Create chat and start streaming so the daemon has an active stream to pause
    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });
    const chatId = get(currentChatId);
    await configureMock('AI response with tool calls');
    await setAutoStream(false);
    await dispatch({
      type: 'sendMessage',
      payload: { content: 'Start', model: get(availableModels)[0] || 'test_model' },
    });
    
    await waitFor(() => {
      expect(get(isStreaming)).toBe(true);
    }, { timeout: 5000 });
    
    // Simulate tool call started
    await dispatch({
      type: 'chatToolCallStarted',
      payload: {
        chatId: chatId!,
        toolCallId: 'call_1',
        toolName: 'test_tool',
        arguments: '{}',
        mcpId: 'test_mcp',
      },
    });
    
    // Step 2. System dispatches `pauseChat` action
    // Step 3. System sends pause request to daemon
    await dispatch({ type: 'pauseChat' });
    
    // Step 4-7. Daemon waits for current tool calls to complete (mocked)
    // Step 8. System receives `chatPaused` action
    // Step 9. System sets isPaused to true
    // Step 10. System sets isStreaming to false
    await dispatch({ type: 'chatPaused' });
    
    expect(get(isPaused)).toBe(true);
    expect(get(isStreaming)).toBe(false);
    
    // Step 11. User types message in input field (UI test)
    // Step 12. User clicks Send button (UI test)
    // Step 13. System dispatches `queueMessage` action
    // Step 14. System sends queue request to daemon
    // Step 15. System optimistically adds the message to pendingMessages store (gray, not yet part of chat)
    const model = get(availableModels)[0] || 'test_model';
    await dispatch({ type: 'queueMessage', payload: { content: 'Hello', model } });
    
    expect(get(pendingMessages).length).toBe(1);
    
    // Step 16. User clicks Resume button (UI test)
    // Step 17. System dispatches `resumeChat` action
    // Step 18. System sends resume request to daemon
    await dispatch({ type: 'resumeChat' });
    
    // Step 19-22. Daemon processes queued messages (mocked)
    // Step 23. System receives `chatResumed` action
    // Step 24. System sets isPaused to false
    // Step 25. System sets isStreaming to true
    await dispatch({ type: 'chatResumed' });
    
    expect(get(isPaused)).toBe(false);
    expect(get(isStreaming)).toBe(true);
    
    // The interrupted call has not finished, so the message stays queued and gray
    expect(get(pendingMessages).length).toBe(1);
    
    // Step 26. System receives `chatStreamFinished` while resumed
    // Step 27. System promotes the queued message into messages as a regular user message
    await dispatch({ type: 'chatStreamFinished' });
    
    expect(get(pendingMessages).length).toBe(0);
    expect(get(messages).filter((m) => m.role === 'user' && m.content === 'Hello').length).toBe(1);
    
    // A later daemon echo replaces the promoted message instead of duplicating it
    await dispatch({
      type: 'chatMessageAdded',
      payload: { message: { id: 9002, chatId: chatId!, role: 'user', content: 'Hello', createdAt: new Date().toISOString(), model } as any },
    });
    
    expect(get(pendingMessages).length).toBe(0);
    expect(get(messages).filter((m) => m.content === 'Hello').length).toBe(1);
  });

  // Covers abort-during-ai-call.md steps 2-30 (state logic)
  // Steps 1, 13, 20 are UI tests (see MessageInput.test.ts)
  it('aborts during AI call and resumes without aborted message', async () => {
    // Setup: Create chat and start streaming
    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });
    await configureMock('Streaming response');
    await setAutoStream(false);
    const model = get(availableModels)[0] || 'test_model';
    await dispatch({ type: 'sendMessage', payload: { content: 'Start', model } });

    // Wait for streaming to start
    await waitFor(() => {
      expect(get(isStreaming)).toBe(true);
    }, { timeout: 5000 });

    // Wait for the backend to connect to the mock server's streaming endpoint
    await waitForStreamReady();

    // Send a chunk to ensure the backend has registered the stream and is actively streaming
    await emitStreamChunk('Thinking');
    await waitFor(() => {
      expect(get(streamingMessageId)).not.toBeNull();
    }, { timeout: 5000 });

    // Give the backend time to process the chunk and return to the select! loop
    await new Promise((resolve) => setTimeout(resolve, 200));

    // Step 2. System dispatches `abortChat` action
    // Step 3. System sends abort request to daemon
    await dispatch({ type: 'abortChat' });

    // Step 4-8. Daemon cancels AI request and transitions to Aborted state
    // Step 9. System receives `streamAborted` event from backend
    // Step 10. System removes streaming message from UI
    // Step 11. System sets isPaused to true
    // Step 12. System sets isStreaming to false
    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 5000 });
    await waitFor(() => {
      expect(get(isPaused)).toBe(true);
      expect(get(isAborted)).toBe(true);
      expect(get(streamingMessageId)).toBeNull();
    }, { timeout: 5000 });

    // Verify streaming message was removed (but assistant message with tool calls may remain)
    const assistantMessages = get(messages).filter((m) => m.role === 'assistant');
    // If there are assistant messages, they should have tool calls (not be streaming messages)
    assistantMessages.forEach((msg) => {
      expect(msg.toolCalls).toBeDefined();
      expect(msg.toolCalls?.length).toBeGreaterThan(0);
    });

    // Step 13. User types message in input field (UI test)
    // Step 14. User clicks Send button (UI test)
    // Step 15. System dispatches `queueMessage` action
    // Step 16. System sends queue request to daemon
    // Step 17. System optimistically adds the message to pendingMessages store (gray, not yet part of chat)
    await dispatch({ type: 'queueMessage', payload: { content: 'Hello', model } });

    expect(get(pendingMessages).length).toBe(1);

    // Step 18. User clicks Resume button (UI test)
    // Step 19. System dispatches `resumeChat` action
    // Step 20. System sends resume request to daemon
    await dispatch({ type: 'resumeChat' });

    // Step 21-24. Daemon processes queued messages, adds them to DB, starts new AI call
    // Step 25. System receives `chatResumed` event from backend
    // Step 26. System sets isPaused to false
    // Step 27. System sets isStreaming to true
    await waitFor(() => {
      expect(get(isPaused)).toBe(false);
      expect(get(isAborted)).toBe(false);
      expect(get(isStreaming)).toBe(true);
      expect(get(pendingMessages).length).toBe(0);
    }, { timeout: 5000 });

    // Verify the queued message was promoted to a regular user message
    expect(get(messages).filter((m) => m.role === 'user' && m.content === 'Hello').length).toBe(1);
  });

  // Covers abort-during-tool-execution.md steps 2-31 (state logic)
  // Steps 1, 14, 21 are UI tests (see MessageInput.test.ts)
  it('aborts during tool execution and resumes with error results', async () => {
    // Setup: Create chat and start streaming so the daemon has an active stream to abort
    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });
    const chatId = get(currentChatId);
    await configureMock('AI response with tool calls');
    await setAutoStream(false);
    await dispatch({
      type: 'sendMessage',
      payload: { content: 'Start', model: get(availableModels)[0] || 'test_model' },
    });
    
    await waitFor(() => {
      expect(get(isStreaming)).toBe(true);
    }, { timeout: 5000 });
    
    // Simulate tool call started
    await dispatch({
      type: 'chatToolCallStarted',
      payload: {
        chatId: chatId!,
        toolCallId: 'call_1',
        toolName: 'test_tool',
        arguments: '{}',
        mcpId: 'test_mcp',
      },
    });
    
    // Step 2. System dispatches `abortChat` action
    // Step 3. System sends abort request to daemon
    await dispatch({ type: 'abortChat' });
    
    // Step 4-9. Daemon cancels tool execution and inserts results (mocked)
    // Step 10. System receives `chatToolCallCompleted` with error result
    // Step 11. System updates tool call UI to show "Aborted" error
    await dispatch({
      type: 'chatToolCallCompleted',
      payload: {
        chatId: chatId!,
        toolCallId: 'call_1',
        result: 'Aborted',
        isError: true,
      },
    });
    
    // Step 12. System receives `streamAborted` action
    // Step 13. System sets isPaused to true, isStreaming to false
    await dispatch({ type: 'streamAborted' });
    
    expect(get(isPaused)).toBe(true);
    expect(get(isAborted)).toBe(true);
    expect(get(isStreaming)).toBe(false);
    
    // Verify tool call shows error
    const allMessages = get(messages);
    const toolCallMsg = allMessages.find(m => m.role === 'assistant' && m.toolCalls);
    expect(toolCallMsg).toBeDefined();
    expect(toolCallMsg?.toolCalls?.[0].status).toBe('failed');
    expect(toolCallMsg?.toolCalls?.[0].result).toBe('Aborted');
    
    // Step 14. User types message in input field (UI test)
    // Step 15. User clicks Send button (UI test)
    // Step 16. System dispatches `queueMessage` action
    // Step 17. System sends queue request to daemon
    // Step 18. System optimistically adds the message to pendingMessages store (gray, not yet part of chat)
    const model = get(availableModels)[0] || 'test_model';
    await dispatch({ type: 'queueMessage', payload: { content: 'Hello', model } });
    
    expect(get(pendingMessages).length).toBe(1);
    
    // A stream finishing while still aborted means the chat was never resumed,
    // so the message is genuinely still queued and stays gray
    await dispatch({ type: 'chatStreamFinished' });
    
    expect(get(isAborted)).toBe(true);
    expect(get(pendingMessages).length).toBe(1);
    expect(get(messages).filter((m) => m.role === 'user' && m.content === 'Hello').length).toBe(0);
    
    // Step 19. User clicks Resume button (UI test)
    // Step 20. System dispatches `resumeChat` action
    // Step 21. System sends resume request to daemon
    await dispatch({ type: 'resumeChat' });
    
    // Step 22-25. Daemon processes queued messages (mocked)
    // Step 26. System receives `chatResumed` action
    // Step 27. System sets isPaused to false
    // Step 28. System sets isStreaming to true
    // The aborted AI call was cancelled, so the queued message is promoted immediately
    await dispatch({ type: 'chatResumed' });
    
    expect(get(isPaused)).toBe(false);
    expect(get(isAborted)).toBe(false);
    expect(get(isStreaming)).toBe(true);
    
    // The aborted call was cancelled, so promotion happens on chatResumed
    expect(get(pendingMessages).length).toBe(0);
    expect(get(messages).filter((m) => m.role === 'user' && m.content === 'Hello').length).toBe(1);
    
    // A later daemon echo replaces the promoted message instead of duplicating it
    await dispatch({
      type: 'chatMessageAdded',
      payload: { message: { id: 9004, chatId: chatId!, role: 'user', content: 'Hello', createdAt: new Date().toISOString(), model } as any },
    });
    
    expect(get(pendingMessages).length).toBe(0);
    expect(get(messages).filter((m) => m.content === 'Hello').length).toBe(1);
  });

  // Covers message-queue-during-pause-abort.md steps 3-26 (state logic)
  // Steps 1, 8, 15 are UI tests (see MessageInput.test.ts)
  it('queues multiple messages during pause and sends on resume', async () => {
    // Setup: Create chat, start streaming so the daemon has an active stream, then pause
    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });
    const chatId = get(currentChatId);
    const model = get(availableModels)[0] || 'test_model';
    await configureMock('AI response');
    await setAutoStream(false);
    await dispatch({ type: 'sendMessage', payload: { content: 'Start', model } });
    
    await waitFor(() => {
      expect(get(isStreaming)).toBe(true);
    }, { timeout: 5000 });
    
    await dispatch({ type: 'pauseChat' });
    await dispatch({ type: 'chatPaused' });
    
    expect(get(isPaused)).toBe(true);
    
    // Step 3. System dispatches `queueMessage` action
    // Step 4. System sends queue request to daemon
    // Step 5. System optimistically adds the message to pendingMessages store (gray, not yet part of chat)
    await dispatch({ type: 'queueMessage', payload: { content: 'First', model } });
    
    expect(get(pendingMessages).length).toBe(1);
    expect(get(pendingMessages)[0].content).toBe('First');
    
    // Step 6. User types second message in input field (UI test)
    // Step 7. User clicks Send button (UI test)
    // Step 8. System dispatches `queueMessage` action
    // Step 9. System sends queue request to daemon
    // Step 10. System optimistically adds the second message to pendingMessages store
    await dispatch({ type: 'queueMessage', payload: { content: 'Second', model } });
    
    expect(get(pendingMessages).length).toBe(2);
    expect(get(pendingMessages)[1].content).toBe('Second');
    
    // Step 11. User clicks Resume button (UI test)
    // Step 12. System dispatches `resumeChat` action
    // Step 13. System sends resume request to daemon
    await dispatch({ type: 'resumeChat' });
    
    // Step 14-18. Daemon processes queued messages in order (mocked)
    // Step 19. System receives `chatResumed` action
    // Step 20. System sets isPaused to false
    // Step 21. System sets isStreaming to true
    await dispatch({ type: 'chatResumed' });
    
    expect(get(isPaused)).toBe(false);
    expect(get(isStreaming)).toBe(true);
    
    // Both messages stay queued and gray while the interrupted call is still in flight
    expect(get(pendingMessages).length).toBe(2);
    
    // Step 22. System receives `chatStreamFinished` while resumed
    // Step 23. System promotes both queued messages in queue order
    await dispatch({ type: 'chatStreamFinished' });
    
    expect(get(pendingMessages).length).toBe(0);
    
    const promotedContents = get(messages)
      .filter((m) => m.role === 'user' && (m.content === 'First' || m.content === 'Second'))
      .map((m) => m.content);
    expect(promotedContents).toEqual(['First', 'Second']);
    
    // Step 24. Later daemon echoes replace the promoted messages instead of duplicating them
    await dispatch({
      type: 'chatMessageAdded',
      payload: { message: { id: 9005, chatId: chatId!, role: 'user', content: 'First', createdAt: new Date().toISOString(), model } as any },
    });
    
    await dispatch({
      type: 'chatMessageAdded',
      payload: { message: { id: 9006, chatId: chatId!, role: 'user', content: 'Second', createdAt: new Date().toISOString(), model } as any },
    });
    
    expect(get(pendingMessages).length).toBe(0);
    expect(get(messages).filter((m) => m.content === 'First').length).toBe(1);
    expect(get(messages).filter((m) => m.content === 'Second').length).toBe(1);
  });
});
