import type Database from 'better-sqlite3';
import type {
  ChatDetail,
  ChatSummary,
  ConversationTurn,
  RequestDetail,
  RequestSummary
} from '../lib/api/schemas.js';

/**
 * All SQL lives here. READ-ONLY BY CONSTRUCTION: every statement is a SELECT —
 * this module is the enforcement point of the viewer's read-only guarantee.
 */

interface ChatRow {
  id: number;
  title: string;
  model: string | null;
  created_at: string;
  updated_at: string;
  request_count: number;
}

interface RequestRow {
  id: number;
  chat_id: number;
  ts: string;
  method: string;
  path: string;
  model: string | null;
  stream: number;
  status: number | null;
  duration_ms: number | null;
  error: string | null;
  has_assembled: number;
}

interface RequestDetailRow extends RequestRow {
  request_body: Uint8Array;
  response_body: Uint8Array | null;
  response_assembled: string | null;
}

function toChatSummary(row: ChatRow): ChatSummary {
  return {
    id: row.id,
    title: row.title,
    model: row.model,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    requestCount: row.request_count
  };
}

function toRequestSummary(row: RequestRow): RequestSummary {
  return {
    id: row.id,
    chatId: row.chat_id,
    ts: row.ts,
    method: row.method,
    path: row.path,
    model: row.model,
    stream: row.stream === 1,
    status: row.status,
    durationMs: row.duration_ms,
    error: row.error,
    hasAssembled: row.has_assembled === 1
  };
}

function blobToText(bytes: Uint8Array | null): string | null {
  return bytes === null ? null : Buffer.from(bytes).toString('utf-8');
}

/** Chat summaries with request counts, most recently active first. */
export function listChats(db: Database.Database): ChatSummary[] {
  const stmt = db.prepare(`
    SELECT c.id, c.title, c.model, c.created_at, c.updated_at,
           (SELECT COUNT(*) FROM requests r WHERE r.chat_id = c.id) AS request_count
    FROM chats c
    ORDER BY c.updated_at DESC, c.id DESC
  `);
  return stmt.all().map((row) => toChatSummary(row as ChatRow));
}

/** Request summaries of one chat, oldest first. Returns null when the chat does not exist. */
export function listRequests(db: Database.Database, chatId: number): RequestSummary[] | null {
  const chat = db
    .prepare('SELECT id FROM chats WHERE id = ?')
    .get(chatId) as { id: number } | undefined;
  if (!chat) {
    return null;
  }
  const stmt = db.prepare(`
    SELECT id, chat_id, ts, method, path, model, stream, status, duration_ms, error,
           response_assembled IS NOT NULL AS has_assembled
    FROM requests
    WHERE chat_id = ?
    ORDER BY id ASC
  `);
  return stmt.all(chatId).map((row) => toRequestSummary(row as RequestRow));
}

/**
 * Conversation view of a chat: the latest request's `messages` history as
 * message turns, followed by one assistant turn describing that request's
 * outcome.
 *
 * Rationale (avoids duplicated assistant turns): append-only harnesses embed
 * earlier assistant replies in each request's history, so the latest history
 * already IS the full conversation; only the final reply must come from the
 * log. Per-request assembled replies stay reachable via RequestDetailView.
 */
export function buildConversation(
  db: Database.Database,
  chatId: number
): ConversationTurn[] {
  const latest = db.prepare(`
    SELECT r.id, r.status, r.error, r.response_assembled, raw.request_body
    FROM requests r
    JOIN raw ON raw.request_id = r.id
    WHERE r.chat_id = ?
    ORDER BY r.id DESC
    LIMIT 1
  `).get(chatId) as
    | { id: number; status: number | null; error: string | null;
        response_assembled: string | null; request_body: Uint8Array }
    | undefined;

  if (!latest) {
    return [];
  }

  const turns: ConversationTurn[] = [];

  // Message spine: parse the latest request body and map its messages array.
  try {
    const body: unknown = JSON.parse(blobToText(latest.request_body) ?? '');
    if (body !== null && typeof body === 'object' && Array.isArray((body as { messages?: unknown }).messages)) {
      for (const message of (body as { messages: unknown[] }).messages) {
        if (message !== null && typeof message === 'object' && 'role' in message) {
          const record = message as { role: unknown; content?: unknown };
          if (typeof record.role === 'string') {
            turns.push({ kind: 'message', role: record.role, content: record.content ?? null });
          }
        }
      }
    }
  } catch {
    // Not JSON (or not an object): spine stays empty, assistant turn still shown.
  }

  turns.push({
    kind: 'assistant',
    requestId: latest.id,
    content: latest.response_assembled,
    error: latest.error,
    pending: latest.status === null && latest.error === null
  });

  return turns;
}

/** Full chat detail (summary + requests + conversation). Null when the chat does not exist. */
export function getChatDetail(db: Database.Database, chatId: number): ChatDetail | null {
  const requests = listRequests(db, chatId);
  if (requests === null) {
    return null;
  }
  const summaryRow = db.prepare(`
    SELECT c.id, c.title, c.model, c.created_at, c.updated_at,
           (SELECT COUNT(*) FROM requests r WHERE r.chat_id = c.id) AS request_count
    FROM chats c
    WHERE c.id = ?
  `).get(chatId) as ChatRow | undefined;
  // summaryRow exists because listRequests found the chat.
  return {
    chat: toChatSummary(summaryRow!),
    requests,
    conversation: buildConversation(db, chatId)
  };
}

/** Full request detail incl. raw bodies. Null when the request does not exist. */
export function getRequestDetail(db: Database.Database, requestId: number): RequestDetail | null {
  const row = db.prepare(`
    SELECT r.id, r.chat_id, r.ts, r.method, r.path, r.model, r.stream, r.status,
           r.duration_ms, r.error, r.response_assembled IS NOT NULL AS has_assembled,
           r.response_assembled,
           raw.request_body, raw.response_body
    FROM requests r
    JOIN raw ON raw.request_id = r.id
    WHERE r.id = ?
  `).get(requestId) as RequestDetailRow | undefined;

  if (!row) {
    return null;
  }
  return {
    summary: toRequestSummary(row),
    requestBody: blobToText(row.request_body)!,
    responseBody: blobToText(row.response_body),
    responseAssembled: row.response_assembled
  };
}
