import { z } from 'zod';

// ============================================================================
// Data Structure Schemas
// ============================================================================

/**
 * Chat summary schema.
 * Used in chat lists and chat events.
 */
export const ChatSchema = z.object({
  id: z.number(),
  title: z.string(),
  createdAt: z.string(), // ISO 8601 date string
  updatedAt: z.string(), // ISO 8601 date string
  tags: z.array(z.string()),
  version: z.number()
});

/**
 * Function call schema.
 * Represents a function call within a tool call.
 */
export const FunctionCallSchema = z.object({
  name: z.string(),
  arguments: z.string()
});

/**
 * Tool call schema.
 * Represents a tool call made by the assistant.
 */
export const ToolCallSchema = z.object({
  id: z.string(),
  type: z.string(),
  function: FunctionCallSchema,
  tags: z.array(z.string()).default([])
});

/**
 * Message schema.
 * Used for both regular messages and queue messages.
 */
export const MessageSchema = z.object({
  id: z.number(),
  chatId: z.number(),
  role: z.string(),
  content: z.string(),
  createdAt: z.string(), // ISO 8601 date string
  reasoningContent: z.string().optional(),
  tags: z.array(z.string()).default([]),
  isFinished: z.boolean().default(true),
  isStreaming: z.boolean().default(false),
  toolCallId: z.string().optional(),
  toolCalls: z.array(ToolCallSchema).default([])
});

/**
 * Plugin summary schema.
 * Used in plugin list and plugin events.
 */
export const PluginSummarySchema = z.object({
  pluginId: z.string(),
  isActive: z.boolean()
});

// ============================================================================
// WebSocket Protocol Schemas
// ============================================================================

/**
 * Request schema (client → server).
 */
export const RequestSchema = z.object({
  type: z.literal('request'),
  id: z.string(),
  method: z.string(),
  params: z.any().default({})
});

/**
 * Response schema (server → client).
 */
export const ResponseSchema = z.object({
  type: z.literal('response'),
  id: z.string(),
  success: z.boolean(),
  data: z.any()
});

/**
 * Event schema (server → client).
 */
export const EventSchema = z.object({
  type: z.literal('event'),
  event: z.string(),
  data: z.any()
});

/**
 * Generic WebSocket message schema.
 * Used to parse incoming messages and determine their type.
 */
export const WebSocketMessageSchema = z.discriminatedUnion('type', [
  ResponseSchema,
  EventSchema
]);

// ============================================================================
// Event Data Schemas
// ============================================================================

/**
 * Chat created event data.
 */
export const ChatCreatedDataSchema = z.object({
  chat: ChatSchema,
  chatVersion: z.number()
});

/**
 * Chat updated event data.
 */
export const ChatUpdatedDataSchema = z.object({
  chat: ChatSchema,
  chatVersion: z.number()
});

/**
 * Chat deleted event data.
 */
export const ChatDeletedDataSchema = z.object({
  chatId: z.number(),
  chatVersion: z.number()
});

/**
 * Message added event data.
 */
export const MessageAddedDataSchema = z.object({
  chatId: z.number(),
  message: MessageSchema,
  chatVersion: z.number()
});

/**
 * Message updated event data.
 */
export const MessageUpdatedDataSchema = z.object({
  chatId: z.number(),
  message: MessageSchema,
  chatVersion: z.number()
});

/**
 * Message deleted event data.
 */
export const MessageDeletedDataSchema = z.object({
  chatId: z.number(),
  messageId: z.number(),
  chatVersion: z.number()
});

/**
 * Queue message added event data.
 */
export const QueueMessageAddedDataSchema = z.object({
  chatId: z.number(),
  message: MessageSchema,
  chatVersion: z.number()
});

/**
 * Queue message updated event data.
 */
export const QueueMessageUpdatedDataSchema = z.object({
  chatId: z.number(),
  message: MessageSchema,
  chatVersion: z.number()
});

/**
 * Queue message deleted event data.
 */
export const QueueMessageDeletedDataSchema = z.object({
  chatId: z.number(),
  messageId: z.number(),
  chatVersion: z.number()
});

/**
 * Plugin registered event data.
 */
export const PluginRegisteredDataSchema = z.object({
  plugin: PluginSummarySchema
});

/**
 * Plugin updated event data.
 */
export const PluginUpdatedDataSchema = z.object({
  plugin: PluginSummarySchema
});

/**
 * Plugin removed event data.
 */
export const PluginRemovedDataSchema = z.object({
  pluginId: z.string()
});

// ============================================================================
// Stream Event Data Schemas
// ============================================================================

/**
 * Stream tool call delta schema.
 */
export const StreamToolCallDeltaSchema = z.object({
  id: z.string(),
  name: z.string(),
  arguments: z.string()
});

/**
 * Stream chunk event data schema.
 */
export const StreamChunkDataSchema = z.object({
  chatId: z.number(),
  type: z.enum(['reasoningDelta', 'contentDelta', 'toolCallDelta']),
  content: z.string().optional(),
  toolCalls: z.array(StreamToolCallDeltaSchema).optional()
});

/**
 * Stream finished event data schema.
 */
export const StreamFinishedDataSchema = z.object({
  chatId: z.number()
});

/**
 * Stream subscribe result schema.
 */
export const StreamSubscribeResultSchema = z.object({
  reasoningContent: z.string(),
  content: z.string(),
  toolCalls: z.array(StreamToolCallDeltaSchema).default([]),
  isFinished: z.boolean()
});

// ============================================================================
// Method Result Schemas
// ============================================================================

/**
 * List chats result.
 */
export const ListChatsResultSchema = z.object({
  chats: z.array(ChatSchema)
});

/**
 * Create chat result.
 */
export const CreateChatResultSchema = z.object({
  chatId: z.number()
});

/**
 * Get chat result.
 */
export const GetChatResultSchema = z.object({
  chat: ChatSchema,
  messages: z.array(MessageSchema)
});

/**
 * Get queue messages result.
 */
export const GetQueueMessagesResultSchema = z.object({
  messages: z.array(MessageSchema)
});

/**
 * Get plugins result.
 */
export const GetPluginsResultSchema = z.object({
  plugins: z.array(PluginSummarySchema)
});

/**
 * Update chat params schema.
 */
export const UpdateChatParamsSchema = z.object({
  chatId: z.number(),
  title: z.string().optional(),
  addTags: z.array(z.string()).optional(),
  removeTags: z.array(z.string()).optional()
});

/**
 * Update chat result schema.
 */
export const UpdateChatResultSchema = z.object({});

// ============================================================================
// Tool Schemas
// ============================================================================

/**
 * Function definition schema.
 * Used within tool definitions.
 */
export const FunctionDefinitionSchema = z.object({
  name: z.string(),
  description: z.string(),
  parameters: z.any()
});

/**
 * Tool definition schema.
 * Matches the OpenAI-style tool format.
 */
export const ToolDefinitionSchema = z.object({
  type: z.string(),
  function: FunctionDefinitionSchema
});

/**
 * Tool info schema.
 * Includes the tool definition and the plugin that registered it.
 */
export const ToolInfoSchema = z.object({
  pluginId: z.string(),
  tool: ToolDefinitionSchema
});

/**
 * Get tools result schema.
 */
export const GetToolsResultSchema = z.object({
  tools: z.array(ToolInfoSchema)
});

/**
 * Tools updated event data schema.
 * Emitted when tools are added or removed from a chat.
 */
export const ToolsUpdatedDataSchema = z.object({
  chatId: z.number(),
  tools: z.array(ToolInfoSchema),
  chatVersion: z.number()
});

// ============================================================================
// Type Exports
// ============================================================================

export type Chat = z.infer<typeof ChatSchema>;
export type Message = z.infer<typeof MessageSchema>;
export type FunctionCall = z.infer<typeof FunctionCallSchema>;
export type ToolCall = z.infer<typeof ToolCallSchema>;
export type PluginSummary = z.infer<typeof PluginSummarySchema>;
export type Request = z.infer<typeof RequestSchema>;
export type Response = z.infer<typeof ResponseSchema>;
export type Event = z.infer<typeof EventSchema>;
export type WebSocketMessage = z.infer<typeof WebSocketMessageSchema>;
export type ListChatsResult = z.infer<typeof ListChatsResultSchema>;
export type CreateChatResult = z.infer<typeof CreateChatResultSchema>;
export type GetChatResult = z.infer<typeof GetChatResultSchema>;
export type GetQueueMessagesResult = z.infer<typeof GetQueueMessagesResultSchema>;
export type GetPluginsResult = z.infer<typeof GetPluginsResultSchema>;
export type UpdateChatParams = z.infer<typeof UpdateChatParamsSchema>;
export type UpdateChatResult = z.infer<typeof UpdateChatResultSchema>;

export type ChatCreatedData = z.infer<typeof ChatCreatedDataSchema>;
export type ChatUpdatedData = z.infer<typeof ChatUpdatedDataSchema>;
export type ChatDeletedData = z.infer<typeof ChatDeletedDataSchema>;
export type MessageAddedData = z.infer<typeof MessageAddedDataSchema>;
export type MessageUpdatedData = z.infer<typeof MessageUpdatedDataSchema>;
export type MessageDeletedData = z.infer<typeof MessageDeletedDataSchema>;
export type QueueMessageAddedData = z.infer<typeof QueueMessageAddedDataSchema>;
export type QueueMessageUpdatedData = z.infer<typeof QueueMessageUpdatedDataSchema>;
export type QueueMessageDeletedData = z.infer<typeof QueueMessageDeletedDataSchema>;
export type PluginRegisteredData = z.infer<typeof PluginRegisteredDataSchema>;
export type PluginUpdatedData = z.infer<typeof PluginUpdatedDataSchema>;
export type PluginRemovedData = z.infer<typeof PluginRemovedDataSchema>;
export type StreamChunkData = z.infer<typeof StreamChunkDataSchema>;
export type StreamFinishedData = z.infer<typeof StreamFinishedDataSchema>;
export type StreamToolCallDelta = z.infer<typeof StreamToolCallDeltaSchema>;
export type StreamSubscribeResult = z.infer<typeof StreamSubscribeResultSchema>;

export type FunctionDefinition = z.infer<typeof FunctionDefinitionSchema>;
export type ToolDefinition = z.infer<typeof ToolDefinitionSchema>;
export type ToolInfo = z.infer<typeof ToolInfoSchema>;
export type GetToolsResult = z.infer<typeof GetToolsResultSchema>;
export type ToolsUpdatedData = z.infer<typeof ToolsUpdatedDataSchema>;
