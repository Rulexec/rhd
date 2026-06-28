import { z } from 'zod';

export const ActiveScenarioSchema = z.object({
  id: z.string(),
  scenarioName: z.string(),
  startedAt: z.string(),
  currentStep: z.string().nullable(),
  currentStepStartedAt: z.string().nullable(),
  promptTokens: z.number(),
  completionTokens: z.number(),
});
export type ActiveScenario = z.infer<typeof ActiveScenarioSchema>;

export const FinishedScenarioSchema = z.object({
  id: z.number(),
  scenario: z.string(),
  status: z.enum(['success', 'error', 'aborted', 'executing']),
  finished: z.string(),
  durationMs: z.number(),
  tokens: z.object({
    promptTokens: z.number(),
    completionTokens: z.number(),
  }).optional(),
  cost: z.number().nullable().optional(),
});
export type FinishedScenario = z.infer<typeof FinishedScenarioSchema>;

export const ChatSchema = z.object({
  id: z.number(),
  title: z.string(),
  createdAt: z.string(),
  updatedAt: z.string(),
  activeModel: z.string().nullable(),
});
export type Chat = z.infer<typeof ChatSchema>;

export const ChatMessageSchema = z.object({
  id: z.number(),
  chatId: z.number(),
  role: z.enum(['user', 'assistant']),
  content: z.string(),
  createdAt: z.string(),
  model: z.string().nullable(),
});
export type ChatMessage = z.infer<typeof ChatMessageSchema>;
