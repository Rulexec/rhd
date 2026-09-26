import type { ZodType } from 'zod';
import {
  ApiErrorSchema,
  ChatDetailSchema,
  ChatsResponseSchema,
  RequestDetailSchema
} from './schemas.js';
import type { ChatDetail, ChatsResponse, RequestDetail } from './schemas.js';

/** API failure with the HTTP status (0 = network/dev-server unreachable). */
export class ApiError extends Error {
  readonly status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

async function parseErrorMessage(response: Response): Promise<string> {
  try {
    const body: unknown = await response.json();
    const parsed = ApiErrorSchema.safeParse(body);
    if (parsed.success) {
      return parsed.data.error;
    }
  } catch {
    // Not a JSON error envelope — fall through.
  }
  return `request failed with status ${response.status}`;
}

async function fetchJson<T>(url: string, schema: ZodType<T>): Promise<T> {
  let response: Response;
  try {
    response = await fetch(url);
  } catch (cause) {
    throw new ApiError(
      `dev server unreachable: ${cause instanceof Error ? cause.message : String(cause)}`,
      0
    );
  }
  if (!response.ok) {
    throw new ApiError(await parseErrorMessage(response), response.status);
  }
  const data: unknown = await response.json();
  return schema.parse(data);
}

export function listChats(): Promise<ChatsResponse> {
  return fetchJson('/api/chats', ChatsResponseSchema);
}

export function getChatDetail(chatId: number): Promise<ChatDetail> {
  return fetchJson(`/api/chats/${chatId}`, ChatDetailSchema);
}

export function getRequestDetail(requestId: number): Promise<RequestDetail> {
  return fetchJson(`/api/requests/${requestId}`, RequestDetailSchema);
}
