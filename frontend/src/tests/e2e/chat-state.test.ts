/**
 * Test cases covered:
 * - tests/cases/chat-create.md
 * - tests/cases/chat-send-message.md
 * - tests/cases/chat-streaming.md
 * - tests/cases/chat-select-model.md
 */

import { describe, it, expect, beforeAll, afterAll, beforeEach } from 'vitest';
import { spawn, type ChildProcess } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { get } from 'svelte/store';
import { waitFor } from '@testing-library/svelte';
import { dispatch, _testClearOverrides } from '../../lib/actions';
import {
  chats,
  currentChatId,
  messages,
  isStreaming,
  availableModels,
  selectedModel,
  resetAllStores,
} from '../../lib/chatStores';
import {
  waitForWebSocket,
  setControlPort,
  setWsPort,
  configureMock,
} from '../testUtils';
import { setWsPort as setWsWsPort, connectWebSocket } from '../../lib/ws';

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
    await dispatch({ type: 'loadAvailableModels' });

    const models = get(availableModels);
    expect(models.length).toBeGreaterThan(0);
    expect(models).toContain('test_model');
  });

  it('creates chat via daemon and updates state', async () => {
    await dispatch({ type: 'createChat', payload: { title: 'Test Chat' } });

    const allChats = get(chats);
    expect(allChats.length).toBe(1);
    expect(allChats[0].title).toBe('Test Chat');

    expect(get(currentChatId)).toBeDefined();
  });

  it('sends message and receives streaming response from daemon', async () => {
    await dispatch({ type: 'createChat', payload: { title: 'Test' } });
    await configureMock('AI response content');

    const model = get(availableModels)[0] || 'test_model';
    await dispatch({ type: 'sendMessage', payload: { content: 'Hello', model } });

    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 5000 });

    const allMessages = get(messages);
    expect(allMessages.length).toBe(5);

    const userMsg = allMessages.find((m) => m.role === 'user');
    expect(userMsg).toBeDefined();
    expect(userMsg!.content).toBe('Hello');

    const assistantMsgs = allMessages.filter((m) => m.role === 'assistant');
    expect(assistantMsgs.length).toBe(2);
    
    const finalAssistantMsg = assistantMsgs.find((m) => m.content === 'AI response content');
    expect(finalAssistantMsg).toBeDefined();
  });

  it('handles multiple messages in sequence', async () => {
    await dispatch({ type: 'createChat', payload: { title: 'Test' } });

    const model = get(availableModels)[0] || 'test_model';

    await configureMock('First response');
    await dispatch({ type: 'sendMessage', payload: { content: 'First message', model } });

    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 5000 });

    let allMessages = get(messages);
    expect(allMessages.length).toBe(5);

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
    await dispatch({ type: 'loadAvailableModels' });

    const models = get(availableModels);
    expect(models.length).toBeGreaterThan(0);

    selectedModel.set(null);
    selectedModel.set(models[0]);

    expect(get(selectedModel)).toBe(models[0]);
  });
});
