import { z } from 'zod';
import { ChatMessageSchema, FinishedScenarioSchema } from './index';

export const WsResponseSchema = z.object({
  id: z.string(),
  type: z.literal('response'),
  success: z.boolean(),
  data: z.any().optional(),
  error: z.string().optional(),
});
export type WsResponse = z.infer<typeof WsResponseSchema>;

export const ScenarioStartedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('scenarioStarted'),
  data: z.object({
    id: z.union([z.string(), z.number()]),
    name: z.string(),
    startedAt: z.string(),
  }),
});

export const StepStartedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('stepStarted'),
  data: z.object({
    executionId: z.union([z.string(), z.number()]),
    stepName: z.string(),
    startedAt: z.string(),
  }),
});

export const ScenarioFinishedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('scenarioFinished'),
  data: FinishedScenarioSchema,
});

export const ChatStreamChunkEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatStreamChunk'),
  data: z.object({
    content: z.string(),
  }),
});

export const ChatThinkingChunkEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatThinkingChunk'),
  data: z.object({
    content: z.string(),
  }),
});

export const ChatStreamFinishedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatStreamFinished'),
  data: z.object({
    messageId: z.number(),
    chatId: z.number(),
  }),
});

export const ChatStreamErrorEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatStreamError'),
  data: z.object({
    error: z.string(),
  }),
});

export const ChatMessageAddedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatMessageAdded'),
  data: z.object({
    message: ChatMessageSchema,
  }),
});

export const ChatUpdatedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatUpdated'),
  data: z.object({
    chatId: z.number(),
    title: z.string(),
  }),
});

export const ScenarioPausedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('scenarioPaused'),
  data: z.object({
    executionId: z.union([z.string(), z.number()]),
    scenarioName: z.string(),
    error: z.string(),
    stepName: z.string(),
    availableModels: z.array(z.string()),
  }),
});

export const ScenarioResumedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('scenarioResumed'),
  data: z.object({
    executionId: z.union([z.string(), z.number()]),
  }),
});

export const DevNotificationEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('devNotification'),
  data: z.object({
    title: z.string(),
    message: z.string(),
  }),
});

export const ProjectMcpStatusChangedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('projectMcpStatusChanged'),
  data: z.object({
    projectName: z.string(),
    mcpId: z.string(),
    status: z.enum(['connecting', 'connected', 'failed']),
    error: z.string().optional(),
  }),
});

export const ProjectAttachedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('projectAttached'),
  data: z.object({
    chatId: z.number(),
    projectName: z.string(),
  }),
});

export const ProjectDetachedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('projectDetached'),
  data: z.object({
    chatId: z.number(),
    projectName: z.string(),
  }),
});

export const ToolCallStartedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatToolCallStarted'),
  data: z.object({
    chatId: z.number(),
    toolCallId: z.string(),
    toolName: z.string(),
    arguments: z.string(),
    mcpId: z.string(),
  }),
});

export const ToolCallCompletedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatToolCallCompleted'),
  data: z.object({
    chatId: z.number(),
    toolCallId: z.string(),
    result: z.string(),
  }),
});

export const ChatPausedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatPaused'),
  data: z.object({
    chatId: z.number(),
  }),
});

export const ChatResumedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatResumed'),
  data: z.object({
    chatId: z.number(),
  }),
});

export const RoleInfoSchema = z.object({
  projectName: z.string(),
  roleName: z.string(),
  whenToUse: z.string(),
});

export const RoleChangedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('roleChanged'),
  data: z.object({
    chatId: z.number(),
    projectName: z.string(),
    roleName: z.string(),
  }),
});

export const RolesUpdatedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('rolesUpdated'),
  data: z.object({
    chatId: z.number(),
    roles: z.array(RoleInfoSchema),
    activeRoleProject: z.string().optional(),
    activeRoleName: z.string().optional(),
  }),
});

export const ActiveRoleClearedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('activeRoleCleared'),
  data: z.object({
    chatId: z.number(),
  }),
});

export const TodoItemSchema = z.object({
  content: z.string(),
  status: z.enum(['pending', 'in_progress', 'completed', 'discarded']),
});

export const TodoListUpdatedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('todoListUpdated'),
  data: z.object({
    chatId: z.number(),
    items: z.array(TodoItemSchema),
  }),
});

export const StreamAbortedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('streamAborted'),
  data: z.object({
    chatId: z.number(),
  }),
});

export const MessageQueuedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('messageQueued'),
  data: z.object({
    chatId: z.number(),
    content: z.string(),
    model: z.string(),
  }),
});

export const WsEventSchema = z.discriminatedUnion('event', [
  ScenarioStartedEventSchema,
  StepStartedEventSchema,
  ScenarioFinishedEventSchema,
  ChatStreamChunkEventSchema,
  ChatThinkingChunkEventSchema,
  ChatStreamFinishedEventSchema,
  ChatStreamErrorEventSchema,
  ChatMessageAddedEventSchema,
  ChatUpdatedEventSchema,
  ScenarioPausedEventSchema,
  ScenarioResumedEventSchema,
  DevNotificationEventSchema,
  ProjectMcpStatusChangedEventSchema,
  ProjectAttachedEventSchema,
  ProjectDetachedEventSchema,
  ToolCallStartedEventSchema,
  ToolCallCompletedEventSchema,
  ChatPausedEventSchema,
  ChatResumedEventSchema,
  RoleChangedEventSchema,
  RolesUpdatedEventSchema,
  ActiveRoleClearedEventSchema,
  TodoListUpdatedEventSchema,
  StreamAbortedEventSchema,
  MessageQueuedEventSchema,
]);
export type WsEvent = z.infer<typeof WsEventSchema>;

export const WsMessageSchema = z.union([WsResponseSchema, WsEventSchema]);
export type WsMessage = z.infer<typeof WsMessageSchema>;
