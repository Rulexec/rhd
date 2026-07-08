/**
 * Test cases covered:
 * - tests/cases/chat-mcp-tools.md
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
  chatProjects,
  mcpStatuses,
  attachProject,
  loadProjects,
} from '../../lib/projectStores';
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

describe('Chat with MCP tools', () => {
  beforeEach(() => {
    resetAllStores();
    _testClearOverrides();
  });

  it('attaches project with MCP and streams final response after tool call', async () => {
    await dispatch({ type: 'loadAvailableModels' });
    await loadProjects();

    const models = get(availableModels);
    expect(models.length).toBeGreaterThan(0);
    const model = models[0];

    await dispatch({ type: 'createChat', payload: { title: 'MCP Test Chat' } });
    const chatId = get(currentChatId);
    expect(chatId).toBeDefined();

    const attachResult = await attachProject(chatId!, 'test-project-mcp');
    expect(attachResult.success).toBe(true);

    await waitFor(() => {
      const projects = get(chatProjects);
      expect(projects.some((p) => p.name === 'test-project-mcp')).toBe(true);
    }, { timeout: 5000 });

    await waitFor(() => {
      const statuses = get(mcpStatuses);
      const mockMcpStatus = statuses.find(
        (s) => s.projectName === 'test-project-mcp' && s.mcpId === 'mock1'
      );
      expect(mockMcpStatus).toBeDefined();
      expect(mockMcpStatus?.status).toBe('connected');
    }, { timeout: 10000 });

    await configureMock('Tool executed successfully! The echo tool returned: Echo: Hello MCP');

    await dispatch({
      type: 'sendMessage',
      payload: { content: 'Please use the echo tool with message "Hello MCP"', model },
    });

    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 15000 });

    const allMessages = get(messages);
    expect(allMessages.length).toBeGreaterThanOrEqual(2);

    const userMsg = allMessages.find((m) => m.role === 'user');
    expect(userMsg).toBeDefined();
    expect(userMsg!.content).toContain('echo tool');

    const assistantMsg = allMessages.find((m) => m.role === 'assistant');
    expect(assistantMsg).toBeDefined();
    expect(assistantMsg!.content).toContain('Echo: Hello MCP');
    expect(typeof assistantMsg!.id).toBe('number');
    expect(assistantMsg!.id).toBeGreaterThan(0);
  }, 30000);
});
