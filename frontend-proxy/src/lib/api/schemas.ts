import { z } from 'zod';

/**
 * API contract between the Vite dev-server middleware (src/server/) and the
 * browser client. Single source of truth: the server maps DB rows to these
 * shapes, the client parses responses with the same schemas.
 */

/** One logged chat (chats table row + derived request count). */
export const ChatSummarySchema = z.object({
  id: z.number().int(),
  title: z.string(),
  model: z.string().nullable(),
  createdAt: z.string(),
  updatedAt: z.string(),
  requestCount: z.number().int().nonnegative()
});
export type ChatSummary = z.infer<typeof ChatSummarySchema>;

/** One logged request (requests table row, no bodies). */
export const RequestSummarySchema = z.object({
  id: z.number().int(),
  chatId: z.number().int(),
  ts: z.string(),
  method: z.string(),
  path: z.string(),
  model: z.string().nullable(),
  stream: z.boolean(),
  status: z.number().int().nullable(),
  durationMs: z.number().int().nullable(),
  error: z.string().nullable(),
  hasAssembled: z.boolean()
});
export type RequestSummary = z.infer<typeof RequestSummarySchema>;

/**
 * One turn of the derived conversation view of a chat: a normalized row of the
 * logging `messages` table (schema v2). Projection columns parsed back into
 * values: `content` and `toolCalls` arrive as parsed JSON, not stored text.
 */
export const MessageTurnSchema = z.object({
  kind: z.literal('message'),
  /** 0-based position in the chat's message sequence (messages.seq). */
  seq: z.number().int().nonnegative(),
  role: z.string(),
  /** Captured from a request history or assembled from a response. */
  source: z.enum(['history', 'response']),
  /** OpenAI message content: string or array of parts — kept opaque, rendered client-side. */
  content: z.unknown(),
  /** Parsed tool_calls array (assistant turns); null when the row has none. */
  toolCalls: z.array(z.unknown()).nullable(),
  /** role=tool rows: the tool_call_id they answer. */
  toolCallId: z.string().nullable(),
  /** Optional name field (tool results, named participants). */
  name: z.string().nullable(),
  /** Request whose exchange produced the row. */
  requestId: z.number().int()
});
export type MessageTurn = z.infer<typeof MessageTurnSchema>;

/** Tail state: the chat's latest request is still in flight (no response row yet). */
export const PendingTurnSchema = z.object({
  kind: z.literal('pending'),
  requestId: z.number().int()
});
export type PendingTurn = z.infer<typeof PendingTurnSchema>;

/** Tail state: the chat's latest request failed proxy-side (requests.error). */
export const ErrorTurnSchema = z.object({
  kind: z.literal('error'),
  requestId: z.number().int(),
  error: z.string()
});
export type ErrorTurn = z.infer<typeof ErrorTurnSchema>;

export const ConversationTurnSchema = z.union([
  MessageTurnSchema,
  PendingTurnSchema,
  ErrorTurnSchema
]);
export type ConversationTurn = z.infer<typeof ConversationTurnSchema>;

/** GET /api/chats/:id response. */
export const ChatDetailSchema = z.object({
  chat: ChatSummarySchema,
  /** Oldest first (ORDER BY id ASC). */
  requests: z.array(RequestSummarySchema),
  /** Derived conversation: messages-table rows in seq order + a pending/error tail
   * for the latest request (see queries.ts). */
  conversation: z.array(ConversationTurnSchema)
});
export type ChatDetail = z.infer<typeof ChatDetailSchema>;

/** GET /api/requests/:id response. */
export const RequestDetailSchema = z.object({
  summary: RequestSummarySchema,
  /** Raw request body (pre-injection) as UTF-8 text. */
  requestBody: z.string(),
  /** Raw response body (full SSE bytes for streams) as UTF-8 text; null while in flight. */
  responseBody: z.string().nullable(),
  /** Assembled assistant message as stored (JSON text), null when unparseable/absent. */
  responseAssembled: z.string().nullable()
});
export type RequestDetail = z.infer<typeof RequestDetailSchema>;

/** GET /api/chats response. */
export const ChatsResponseSchema = z.object({
  chats: z.array(ChatSummarySchema)
});
export type ChatsResponse = z.infer<typeof ChatsResponseSchema>;

/** Error envelope for 404/500 responses. */
export const ApiErrorSchema = z.object({
  error: z.string()
});
export type ApiErrorBody = z.infer<typeof ApiErrorSchema>;
