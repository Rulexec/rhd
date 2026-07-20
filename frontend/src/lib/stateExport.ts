import { get } from 'svelte/store';
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
  availableRoles,
  activeRole,
} from './chatStores';
import { projects, mcpStatuses, chatProjects } from './projectStores';
import type { ActiveScenario, FinishedScenario, PausedScenario } from './types/index';
import type { Chat, ChatMessage, ToolCall, Project, McpStatus, ChatProject, RoleInfo, ActiveRole } from './types/index';

export interface ExportedState {
  version: number;
  hash: string;
  scenarios: {
    active: Array<[string, ActiveScenario]>;
    finished: FinishedScenario[];
    paused: Array<[string, PausedScenario]>;
    lastKnownId: number;
    wsConnected: boolean;
  };
  chat: {
    chats: Chat[];
    currentChatId: number | null;
    messages: ChatMessage[];
    streamingContent: string;
    streamingThinkingContent: string;
    isStreaming: boolean;
    streamError: string | null;
    availableModels: string[];
    selectedModel: string | null;
    streamingMessageId: string | null;
    isPaused: boolean;
    pendingToolCalls: ToolCall[];
    availableRoles: RoleInfo[];
    activeRole: ActiveRole | null;
  };
  projects: {
    projects: Project[];
    mcpStatuses: McpStatus[];
    chatProjects: ChatProject[];
  };
}

const CURRENT_VERSION = 1;

export function exportState(): ExportedState {
  return {
    version: CURRENT_VERSION,
    hash: window.location.hash,
    scenarios: {
      active: [...get(activeScenarios).entries()],
      finished: get(finishedScenarios),
      paused: [...get(pausedScenarios).entries()],
      lastKnownId: get(lastKnownId),
      wsConnected: get(wsConnected),
    },
    chat: {
      chats: get(chats),
      currentChatId: get(currentChatId),
      messages: get(messages),
      streamingContent: get(streamingContent),
      streamingThinkingContent: get(streamingThinkingContent),
      isStreaming: get(isStreaming),
      streamError: get(streamError),
      availableModels: get(availableModels),
      selectedModel: get(selectedModel),
      streamingMessageId: get(streamingMessageId),
      isPaused: get(isPaused),
      pendingToolCalls: get(pendingToolCalls),
      availableRoles: get(availableRoles),
      activeRole: get(activeRole),
    },
    projects: {
      projects: get(projects),
      mcpStatuses: get(mcpStatuses),
      chatProjects: get(chatProjects),
    },
  };
}

export function importState(state: unknown): void {
  if (!state || typeof state !== 'object') {
    throw new Error('Invalid state: expected an object');
  }

  const imported = state as Partial<ExportedState>;

  if (imported.version !== CURRENT_VERSION) {
    throw new Error(`Invalid state version: expected ${CURRENT_VERSION}, got ${imported.version}`);
  }

  if (typeof imported.hash !== 'string') {
    throw new Error('Invalid state: hash must be a string');
  }

  if (!imported.scenarios || typeof imported.scenarios !== 'object') {
    throw new Error('Invalid state: scenarios must be an object');
  }

  if (!imported.chat || typeof imported.chat !== 'object') {
    throw new Error('Invalid state: chat must be an object');
  }

  if (!imported.projects || typeof imported.projects !== 'object') {
    throw new Error('Invalid state: projects must be an object');
  }

  const scenarios = imported.scenarios as ExportedState['scenarios'];
  const chat = imported.chat as ExportedState['chat'];
  const projectsState = imported.projects as ExportedState['projects'];

  activeScenarios.setAll(new Map(scenarios.active));
  finishedScenarios.setAll(scenarios.finished);
  pausedScenarios.setAll(new Map(scenarios.paused));
  lastKnownId.set(scenarios.lastKnownId);
  wsConnected.set(scenarios.wsConnected);

  chats.set(chat.chats);
  currentChatId.set(chat.currentChatId);
  messages.set(chat.messages);
  streamingContent.set(chat.streamingContent);
  streamingThinkingContent.set(chat.streamingThinkingContent);
  isStreaming.set(chat.isStreaming);
  streamError.set(chat.streamError);
  availableModels.set(chat.availableModels);
  selectedModel.set(chat.selectedModel);
  streamingMessageId.set(chat.streamingMessageId);
  isPaused.set(chat.isPaused);
  pendingToolCalls.set(chat.pendingToolCalls);
  availableRoles.set(chat.availableRoles);
  activeRole.set(chat.activeRole);

  projects.set(projectsState.projects);
  mcpStatuses.set(projectsState.mcpStatuses);
  chatProjects.set(projectsState.chatProjects);

  window.location.hash = imported.hash;
}

export function setupStateExportImport(): void {
  window.__exportState = exportState;
  window.__importState = importState;
}

declare global {
  interface Window {
    __exportState: () => ExportedState;
    __importState: (state: ExportedState) => void;
  }
}
