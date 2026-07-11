import type { ChatMessage } from '../types/index';

export type ChatAction =
  | { type: 'createChat'; payload: { title: string } }
  | { type: 'selectChat'; payload: { chatId: number } }
  | { type: 'deleteChat'; payload: { chatId: number } }
  | { type: 'deleteAllChats' }
  | { type: 'sendMessage'; payload: { content: string; model: string } }
  | { type: 'editMessage'; payload: { messageId: number; content: string; model: string } }
  | { type: 'abortChat' }
  | { type: 'pauseChat' }
  | { type: 'resumeChat' }
  | { type: 'selectModel'; payload: { model: string } }
  | { type: 'loadChats' }
  | { type: 'loadAvailableModels' }
  | { type: 'chatStreamChunk'; payload: { content: string } }
  | { type: 'chatThinkingChunk'; payload: { content: string } }
  | { type: 'chatStreamFinished' }
  | { type: 'chatStreamError'; payload: { error: string } }
  | { type: 'chatMessageAdded'; payload: { message: ChatMessage } }
  | { type: 'chatUpdated'; payload: { chatId: number; title: string } }
  | { type: 'chatToolCallStarted'; payload: { chatId: number; toolCallId: string; toolName: string; arguments: string; mcpId: string } }
  | { type: 'chatToolCallCompleted'; payload: { chatId: number; toolCallId: string; result: string } }
  | { type: 'chatPaused' }
  | { type: 'chatResumed' }
  | { type: 'projectMcpStatusChanged'; payload: unknown }
  | { type: 'projectAttached'; payload: unknown }
  | { type: 'projectDetached'; payload: unknown };
