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

/** A turn of the derived conversation view of a chat. */
export const MessageTurnSchema = z.object({
  /** Harness message from the latest request's history (system/user/assistant/tool/...). */
  kind: z.literal('message'),
  role: z.string(),
  /** OpenAI message content: string or array of parts — kept opaque, rendered client-side. */
  content: z.unknown()
});
export type MessageTurn = z.infer<typeof MessageTurnSchema>;

export const AssistantTurnSchema = z.object({
  /** Outcome of the chat's latest request, always appended after the message turns. */
  kind: z.literal('assistant'),
  requestId: z.number().int(),
  /** response_assembled as stored (JSON text of the assistant message), null when unparseable. */
  content: z.string().nullable(),
  /** Proxy-side failure text (requests.error), null when the exchange did not fail. */
  error: z.string().nullable(),
  /** True when status IS NULL and error IS NULL — response still in flight. */
  pending: z.boolean()
});
export type AssistantTurn = z.infer<typeof AssistantTurnSchema>;

export const ConversationTurnSchema = z.union([MessageTurnSchema, AssistantTurnSchema]);
export type ConversationTurn = z.infer<typeof ConversationTurnSchema>;

/** GET /api/chats/:id response. */
export const ChatDetailSchema = z.object({
  chat: ChatSummarySchema,
  /** Oldest first (ORDER BY id ASC). */
  requests: z.array(RequestSummarySchema),
  /** Derived conversation: latest request's messages + one assistant turn (see queries.ts). */
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
