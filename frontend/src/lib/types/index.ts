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

export const PausedScenarioSchema = z.object({
  executionId: z.string(),
  scenarioName: z.string(),
  error: z.string(),
  stepName: z.string(),
  availableModels: z.array(z.string()),
  selectedModel: z.string().nullable(),
});
export type PausedScenario = z.infer<typeof PausedScenarioSchema>;

export const ChatSchema = z.object({
  id: z.number(),
  title: z.string(),
  createdAt: z.string(),
  updatedAt: z.string(),
  activeModel: z.string().nullable(),
});
export type Chat = z.infer<typeof ChatSchema>;

export const ToolCallSchema = z.object({
  id: z.string(),
  name: z.string(),
  arguments: z.string(),
  result: z.string().optional(),
  status: z.enum(['pending', 'running', 'completed', 'failed']).optional(),
  mcpId: z.string().optional(),
});
export type ToolCall = z.infer<typeof ToolCallSchema>;

export const ChatMessageSchema = z.object({
  id: z.union([z.number(), z.string()]),
  chatId: z.number(),
  role: z.enum(['user', 'assistant', 'system', 'tool']),
  content: z.string(),
  createdAt: z.string(),
  model: z.string().nullable(),
  toolCalls: z.array(ToolCallSchema).optional(),
  toolCallId: z.string().optional(),
  thinkingContent: z.string().nullish(),
});
export type ChatMessage = z.infer<typeof ChatMessageSchema>;

export const ProjectSchema = z.object({
  name: z.string(),
  hasMcp: z.boolean(),
  hasSystemPrompt: z.boolean(),
});
export type Project = z.infer<typeof ProjectSchema>;

export const McpStatusSchema = z.object({
  projectName: z.string(),
  mcpId: z.string(),
  status: z.enum(['connecting', 'connected', 'failed']),
  error: z.string().optional(),
});
export type McpStatus = z.infer<typeof McpStatusSchema>;

export const ChatProjectSchema = z.object({
  name: z.string(),
  systemPromptAdded: z.boolean(),
});
export type ChatProject = z.infer<typeof ChatProjectSchema>;

export const RoleInfoSchema = z.object({
  projectName: z.string(),
  roleName: z.string(),
  whenToUse: z.string(),
});
export type RoleInfo = z.infer<typeof RoleInfoSchema>;

export const ActiveRoleSchema = z.object({
  projectName: z.string(),
  roleName: z.string(),
});
export type ActiveRole = z.infer<typeof ActiveRoleSchema>;
