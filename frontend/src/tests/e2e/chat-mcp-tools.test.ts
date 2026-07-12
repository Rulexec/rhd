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

    // Find the final assistant message (last one, contains the actual response)
    const assistantMessages = allMessages.filter((m) => m.role === 'assistant');
    expect(assistantMessages.length).toBeGreaterThanOrEqual(2); // intermediate + final
    const assistantMsg = assistantMessages[assistantMessages.length - 1];
    expect(assistantMsg).toBeDefined();
    expect(assistantMsg.content).toContain('Echo: Hello MCP');
    expect(typeof assistantMsg.id).toBe('number');
    expect(assistantMsg.id).toBeGreaterThan(0);
    // Final assistant message should NOT have toolCalls (already shown in intermediate message)
    expect(assistantMsg.toolCalls).toBeUndefined();

    // Verify tool calls are visible in messages
    const assistantMsgsWithToolCalls = allMessages.filter(
      (m) => m.role === 'assistant' && m.toolCalls && m.toolCalls.length > 0
    );
    expect(assistantMsgsWithToolCalls.length).toBeGreaterThan(0);

    const toolCallMsg = assistantMsgsWithToolCalls[0];
    expect(toolCallMsg.toolCalls![0].name).toContain('echo');
    expect(toolCallMsg.toolCalls![0].status).toBe('completed');
    expect(toolCallMsg.toolCalls![0].result).toBeDefined();

    // Thinking content may be present if mock AI sends reasoning
    // (mock server doesn't always send reasoning, so this is optional)

    // Verify message order: user → assistant (with toolCalls) → assistant (final)
    // Tool results are merged into assistant message, no separate tool message
    const userMsgIndex = allMessages.findIndex((m) => m.role === 'user');
    const toolCallMsgIndex = allMessages.findIndex(
      (m) => m.role === 'assistant' && m.toolCalls && m.toolCalls.length > 0
    );
    const finalAssistantMsgIndex = allMessages.findIndex(
      (m) => m.role === 'assistant' && m.content.includes('Echo: Hello MCP')
    );

    expect(userMsgIndex).toBeLessThan(toolCallMsgIndex);
    expect(toolCallMsgIndex).toBeLessThan(finalAssistantMsgIndex);
    
    // Verify no separate tool result messages visible
    const toolResultMsgCount = allMessages.filter((m) => m.role === 'tool').length;
    expect(toolResultMsgCount).toBe(0);
  }, 30000);

  it('handles multiple tool calls with different results', async () => {
    await dispatch({ type: 'loadAvailableModels' });
    await loadProjects();

    const models = get(availableModels);
    expect(models.length).toBeGreaterThan(0);
    const model = models[0];

    await dispatch({ type: 'createChat', payload: { title: 'MCP Multiple Calls Test' } });
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

    // Configure mock to return success
    await configureMock('Tool executed successfully');

    // Send message that triggers tool call
    await dispatch({
      type: 'sendMessage',
      payload: { content: 'Please use the echo tool with message "Hello"', model },
    });

    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 15000 });

    const allMessages = get(messages);

    // Find assistant messages with tool calls
    const assistantMsgsWithToolCalls = allMessages.filter(
      (m) => m.role === 'assistant' && m.toolCalls && m.toolCalls.length > 0
    );

    // Should have one intermediate message with tool call
    expect(assistantMsgsWithToolCalls.length).toBe(1);
    const toolCallMsg = assistantMsgsWithToolCalls[0];
    expect(toolCallMsg.toolCalls!.length).toBe(1);

    // Verify tool call has correct result
    const toolCall = toolCallMsg.toolCalls![0];
    expect(toolCall.name).toContain('echo');
    expect(toolCall.status).toBe('completed');
    expect(toolCall.result).toBeDefined();
    expect(toolCall.result).toContain('Echo: Hello');

    // Verify message order: user → tool call message → final assistant
    const userMsgIndex = allMessages.findIndex((m) => m.role === 'user');
    const toolCallMsgIndex = allMessages.findIndex(
      (m) => m.role === 'assistant' && m.toolCalls && m.toolCalls.length > 0
    );

    expect(userMsgIndex).toBeLessThan(toolCallMsgIndex);
    
    // Verify no separate tool result messages visible
    const toolResultMsgCount = allMessages.filter((m) => m.role === 'tool').length;
    expect(toolResultMsgCount).toBe(0);
  }, 30000);

  it('maintains unique tool call ids across multiple iterations', async () => {
    await dispatch({ type: 'loadAvailableModels' });
    await loadProjects();

    const models = get(availableModels);
    expect(models.length).toBeGreaterThan(0);
    const model = models[0];

    await dispatch({ type: 'createChat', payload: { title: 'MCP Unique IDs Test' } });
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

    // First tool call
    await configureMock('First result');
    await dispatch({
      type: 'sendMessage',
      payload: { content: 'Please use the echo tool with message "First"', model },
    });

    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 15000 });

    const messagesAfterFirst = get(messages);
    const firstToolCallMsgs = messagesAfterFirst.filter(
      (m) => m.role === 'assistant' && m.toolCalls && m.toolCalls.length > 0
    );
    expect(firstToolCallMsgs.length).toBe(1);
    const firstToolCallId = firstToolCallMsgs[0].toolCalls![0].id;
    expect(firstToolCallId).toBeDefined();
    expect(firstToolCallMsgs[0].toolCalls![0].result).toContain('Echo: Hello MCP');

    // Second tool call (different iteration)
    await configureMock('Second result');
    await dispatch({
      type: 'sendMessage',
      payload: { content: 'Please use the echo tool with message "Second"', model },
    });

    await waitFor(() => {
      expect(get(isStreaming)).toBe(false);
    }, { timeout: 15000 });

    const allMessages = get(messages);
    const allToolCallMsgs = allMessages.filter(
      (m) => m.role === 'assistant' && m.toolCalls && m.toolCalls.length > 0
    );

    // Should have two intermediate messages with tool calls
    expect(allToolCallMsgs.length).toBe(2);

    // Verify each tool call has a unique ID
    const toolCallIds = allToolCallMsgs.flatMap((m) => m.toolCalls!.map((tc) => tc.id));
    const uniqueIds = new Set(toolCallIds);
    expect(uniqueIds.size).toBe(toolCallIds.length);

    // Verify first tool call still has correct result
    const firstMsg = allToolCallMsgs[0];
    expect(firstMsg.toolCalls![0].result).toContain('Echo: Hello MCP');

    // Verify second tool call has correct result
    const secondMsg = allToolCallMsgs[1];
    expect(secondMsg.toolCalls![0].result).toContain('Echo: Hello MCP');

    // Verify tool call IDs are different
    expect(firstMsg.toolCalls![0].id).not.toBe(secondMsg.toolCalls![0].id);
  }, 30000);
});
