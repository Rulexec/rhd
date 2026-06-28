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
  event: z.literal('scenariostarted'),
  data: z.object({
    id: z.string(),
    name: z.string(),
    startedAt: z.string(),
  }),
});

export const StepStartedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('stepstarted'),
  data: z.object({
    executionId: z.string(),
    stepName: z.string(),
    startedAt: z.string(),
  }),
});

export const ScenarioFinishedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('scenariofinished'),
  data: FinishedScenarioSchema,
});

export const ChatStreamChunkEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatStreamChunk'),
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

export const WsEventSchema = z.discriminatedUnion('event', [
  ScenarioStartedEventSchema,
  StepStartedEventSchema,
  ScenarioFinishedEventSchema,
  ChatStreamChunkEventSchema,
  ChatStreamFinishedEventSchema,
  ChatStreamErrorEventSchema,
  ChatMessageAddedEventSchema,
  ChatUpdatedEventSchema,
]);
export type WsEvent = z.infer<typeof WsEventSchema>;

export const WsMessageSchema = z.union([WsResponseSchema, WsEventSchema]);
export type WsMessage = z.infer<typeof WsMessageSchema>;
