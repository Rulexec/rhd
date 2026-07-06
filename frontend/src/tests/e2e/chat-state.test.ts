import { describe, it, expect, beforeAll, afterAll, beforeEach } from 'vitest';
import { spawn, type ChildProcess } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { get } from 'svelte/store';
import { waitFor } from '@testing-library/svelte';
import {
  _test_createChat,
  _test_sendMessage,
  _test_loadAvailableModels,
} from '../../lib/chatWs';
import {
  chats,
  currentChatId,
  messages,
  isStreaming,
  availableModels,
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
  });

  it('loads available models from daemon', async () => {
    await _test_loadAvailableModels();

    const models = get(availableModels);
    expect(models.length).toBeGreaterThan(0);
    expect(models).toContain('test_model');
  });

  it('creates chat via daemon and updates state', async () => {
    const response = await _test_createChat('Test Chat');

    expect(response.success).toBe(true);
    expect(response.data.chatId).toBeDefined();

    const allChats = get(chats);
    expect(allChats.length).toBe(1);
    expect(allChats[0].title).toBe('Test Chat');

    expect(get(currentChatId)).toBe(response.data.chatId);
  });

  it('sends message and receives streaming response from daemon', async () => {
    await _test_createChat('Test');
    await configureMock('AI response content');

    await _test_sendMessage('Hello', 'test_model');

    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 5000 });

    const allMessages = get(messages);
    expect(allMessages.length).toBe(2);

    const userMsg = allMessages.find((m) => m.role === 'user');
    expect(userMsg).toBeDefined();
    expect(userMsg!.content).toBe('Hello');

    const assistantMsg = allMessages.find((m) => m.role === 'assistant');
    expect(assistantMsg).toBeDefined();
    expect(assistantMsg!.content).toBe('AI response content');
  });

  it('handles multiple messages in sequence', async () => {
    await _test_createChat('Test');

    await configureMock('First response');
    await _test_sendMessage('First message', 'test_model');

    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 5000 });

    let allMessages = get(messages);
    expect(allMessages.length).toBe(2);

    await configureMock('Second response');
    await _test_sendMessage('Second message', 'test_model');

    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 5000 });

    allMessages = get(messages);
    expect(allMessages.length).toBe(4);

    const userMessages = allMessages.filter((m) => m.role === 'user');
    expect(userMessages.length).toBe(2);
    expect(userMessages[0].content).toBe('First message');
    expect(userMessages[1].content).toBe('Second message');

    const assistantMessages = allMessages.filter((m) => m.role === 'assistant');
    expect(assistantMessages.length).toBe(2);
    expect(assistantMessages[0].content).toBe('First response');
    expect(assistantMessages[1].content).toBe('Second response');
  });
});
