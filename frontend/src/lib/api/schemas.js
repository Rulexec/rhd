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
  tags: z.array(z.string()).default([])
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
  chat: ChatSchema
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

// ============================================================================
// Type Exports (for JSDoc type inference)
// ============================================================================

/** @typedef {z.infer<typeof ChatSchema>} Chat */
/** @typedef {z.infer<typeof MessageSchema>} Message */
/** @typedef {z.infer<typeof PluginSummarySchema>} PluginSummary */
/** @typedef {z.infer<typeof RequestSchema>} Request */
/** @typedef {z.infer<typeof ResponseSchema>} Response */
/** @typedef {z.infer<typeof EventSchema>} Event */
/** @typedef {z.infer<typeof ListChatsResultSchema>} ListChatsResult */
/** @typedef {z.infer<typeof CreateChatResultSchema>} CreateChatResult */
/** @typedef {z.infer<typeof GetChatResultSchema>} GetChatResult */
/** @typedef {z.infer<typeof GetQueueMessagesResultSchema>} GetQueueMessagesResult */
/** @typedef {z.infer<typeof GetPluginsResultSchema>} GetPluginsResult */
