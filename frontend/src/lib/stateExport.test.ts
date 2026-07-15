import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { exportState, importState, setupStateExportImport } from './stateExport';
import {
  activeScenarios,
  finishedScenarios,
  pausedScenarios,
  lastKnownId,
  wsConnected,
} from './stores';
import {
  chats,
  currentChatId,
  messages,
  streamingContent,
  streamingThinkingContent,
  isStreaming,
  streamError,
  availableModels,
  selectedModel,
  streamingMessageId,
  isPaused,
  pendingToolCalls,
  resetAllStores,
} from './chatStores';
import { projects, mcpStatuses, chatProjects } from './projectStores';
import type { ExportedState } from './stateExport';

function resetAllState(): void {
  resetAllStores();
  activeScenarios.setAll(new Map());
  finishedScenarios.setAll([]);
  pausedScenarios.setAll(new Map());
  lastKnownId.set(0);
  wsConnected.set(false);
  projects.set([]);
  mcpStatuses.set([]);
  chatProjects.set([]);
}

function buildSampleState(): ExportedState {
  return {
    version: 1,
    hash: '#/chats/42',
    scenarios: {
      active: [
        [
          'exec-1',
          {
            id: 'exec-1',
            scenarioName: 'Test scenario',
            startedAt: '2026-07-11T10:00:00.000Z',
            currentStep: 'step-1',
            currentStepStartedAt: '2026-07-11T10:01:00.000Z',
            promptTokens: 10,
            completionTokens: 20,
          },
        ],
      ],
      finished: [
        {
          id: 1,
          scenario: 'Finished scenario',
          status: 'success',
          finished: '2026-07-11T10:02:00.000Z',
          durationMs: 1000,
          tokens: { promptTokens: 5, completionTokens: 15 },
          cost: 0.001,
        },
      ],
      paused: [
        [
          'exec-2',
          {
            executionId: 'exec-2',
            scenarioName: 'Paused scenario',
            error: 'paused for user input',
            stepName: 'step-2',
            availableModels: ['gpt-4'],
            selectedModel: null,
          },
        ],
      ],
      lastKnownId: 7,
      wsConnected: true,
    },
    chat: {
      chats: [
        {
          id: 42,
          title: 'Test chat',
          createdAt: '2026-07-11T10:00:00.000Z',
          updatedAt: '2026-07-11T10:01:00.000Z',
          activeModel: 'gpt-4',
        },
      ],
      currentChatId: 42,
      messages: [
        {
          id: 1,
          chatId: 42,
          role: 'user',
          content: 'Hello',
          createdAt: '2026-07-11T10:00:00.000Z',
          model: null,
        },
        {
          id: 2,
          chatId: 42,
          role: 'assistant',
          content: 'Hi there',
          createdAt: '2026-07-11T10:01:00.000Z',
          model: 'gpt-4',
        },
      ],
      streamingContent: 'stream',
      streamingThinkingContent: 'thinking',
      isStreaming: true,
      streamError: 'error',
      availableModels: ['gpt-4', 'gpt-3.5'],
      selectedModel: 'gpt-4',
      streamingMessageId: 'temp-123',
      isPaused: true,
      pendingToolCalls: [
        {
          id: 'tc-1',
          name: 'read_file',
          arguments: '{}',
          status: 'running',
          mcpId: 'mcp-1',
        },
      ],
      availableRoles: [],
      activeRole: null,
    },
    projects: {
      projects: [
        { name: 'project-a', hasMcp: true, hasSystemPrompt: false },
      ],
      mcpStatuses: [
        { projectName: 'project-a', mcpId: 'mcp-1', status: 'connected' },
      ],
      chatProjects: [{ name: 'project-a', systemPromptAdded: true }],
    },
  };
}

describe('stateExport', () => {
  beforeEach(() => {
    resetAllState();
    window.location.hash = '';
    setupStateExportImport();
  });

  it('exports current store values', () => {
    activeScenarios.setAll(new Map([['exec-1', {
      id: 'exec-1',
      scenarioName: 'Active',
      startedAt: '2026-07-11T10:00:00.000Z',
      currentStep: null,
      currentStepStartedAt: null,
      promptTokens: 0,
      completionTokens: 0,
    }]]));
    lastKnownId.set(5);
    currentChatId.set(42);
    window.location.hash = '#/chats/42';

    const state = exportState();

    expect(state.version).toBe(1);
    expect(state.hash).toBe('#/chats/42');
    expect(state.scenarios.lastKnownId).toBe(5);
    expect(state.chat.currentChatId).toBe(42);
    expect(state.scenarios.active).toHaveLength(1);
  });

  it('imports store values and updates hash', () => {
    const state = buildSampleState();

    importState(state);

    expect(get(activeScenarios).get('exec-1')?.scenarioName).toBe('Test scenario');
    expect(get(finishedScenarios)).toHaveLength(1);
    expect(get(pausedScenarios).get('exec-2')?.scenarioName).toBe('Paused scenario');
    expect(get(lastKnownId)).toBe(7);
    expect(get(wsConnected)).toBe(true);

    expect(get(chats)).toHaveLength(1);
    expect(get(currentChatId)).toBe(42);
    expect(get(messages)).toHaveLength(2);
    expect(get(streamingContent)).toBe('stream');
    expect(get(streamingThinkingContent)).toBe('thinking');
    expect(get(isStreaming)).toBe(true);
    expect(get(streamError)).toBe('error');
    expect(get(availableModels)).toEqual(['gpt-4', 'gpt-3.5']);
    expect(get(selectedModel)).toBe('gpt-4');
    expect(get(streamingMessageId)).toBe('temp-123');
    expect(get(isPaused)).toBe(true);
    expect(get(pendingToolCalls)).toHaveLength(1);

    expect(get(projects)).toHaveLength(1);
    expect(get(mcpStatuses)).toHaveLength(1);
    expect(get(chatProjects)).toHaveLength(1);

    expect(window.location.hash).toBe('#/chats/42');
  });

  it('round-trips state identically', () => {
    const original = buildSampleState();
    importState(original);

    const exported = exportState();

    expect(exported).toEqual(original);
  });

  it('rejects invalid state', () => {
    expect(() => importState(null)).toThrow('Invalid state: expected an object');
    expect(() => importState({})).toThrow('Invalid state version');
    expect(() => importState({ version: 1 })).toThrow('Invalid state: hash must be a string');
    expect(() => importState({ version: 1, hash: '#/chats' })).toThrow(
      'Invalid state: scenarios must be an object'
    );
    expect(() => importState({ version: 1, hash: '#/chats', scenarios: {} })).toThrow(
      'Invalid state: chat must be an object'
    );
    expect(() => importState({ version: 1, hash: '#/chats', scenarios: {}, chat: {} })).toThrow(
      'Invalid state: projects must be an object'
    );
  });
});
