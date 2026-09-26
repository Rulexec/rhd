# Phase 2: Vite Middleware — `VITE_PROXY_LOGS_PATH` + SQLite JSON API

## Overview

Turn the dev server into the data layer for the viewer: a Vite plugin installs a connect
middleware that serves a read-only JSON API over `rhd_ai_proxy`'s logging SQLite database.

After this phase there is **no UI yet**, but the API is fully exercisable with `curl`:

- `VITE_PROXY_LOGS_PATH` is validated at config load — the dev server **fails fast** with a clear
  error when it is unset or the database file is missing.
- `GET /api/chats` — chat summaries with request counts, most recently active first.
- `GET /api/chats/:id` — chat detail: summary + request summaries + a server-derived
  **conversation** (message turns from the latest request's history + the final assistant turn).
- `GET /api/requests/:id` — full request detail incl. raw request/response bodies and the
  assembled assistant message.
- Unknown ids → `404` with a JSON error body; database failures → `500` with a JSON error body.

This phase also creates **`src/lib/api/schemas.ts` — the single source of truth for the API
contract** (Zod schemas + inferred types), shared by this server code and the browser client in
Phase 3. Type names defined here (`ChatSummary`, `RequestSummary`, `ChatDetail`, `RequestDetail`,
`ConversationTurn`) are the vocabulary for all later phases.

**Database background (needed to understand the queries):** the logging DB lives at
`<logging dir>/chats.sqlite3` and has tables `chats(id, title, model, created_at, updated_at)`,
`prefix_hashes(hash, chat_id)`, `requests(id, chat_id, ts, method, path, model, stream, status,
duration_ms, error, response_assembled)` and `raw(request_id, request_body, response_body)`.
`status` is `NULL` while a response is in flight; `error` is set when the proxy itself failed the
exchange; `request_body` is the original bytes **before** `extraBody` injection; `response_body`
is the full raw response (concatenated SSE bytes for streams); `response_assembled` is the
reconstructed assistant message as JSON text (nullable). Schema owner:
`packages/rhd_ai_proxy/src/logging/db.rs`.

## Files to Create

### 1. `frontend-proxy/src/lib/api/schemas.ts` — the API contract

```typescript
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
```

### 2. `frontend-proxy/src/server/db.ts` — connection management

```typescript
import fs from 'node:fs';
import path from 'node:path';
import Database from 'better-sqlite3';

/** File name of the logging database inside the configured directory (matches rhd_ai_proxy). */
export const DB_FILE_NAME = 'chats.sqlite3';

/** How long queries wait on the database lock before failing (concurrent proxy writers). */
const BUSY_TIMEOUT_MS = 5000;

/**
 * Handle to the opened logging database.
 *
 * Opened in normal (read-write) mode on purpose: SQLite cannot open a WAL-mode
 * database read-only when no writer is attached (the -shm file needs write
 * access for recovery), and the proxy may well be offline while the viewer is
 * used. Read-only behavior is enforced by the query layer (queries.ts contains
 * only SELECTs). WAL lets the proxy write concurrently while we read.
 */
export interface LogsDb {
  raw: Database.Database;
}

export function openLogsDb(logsDir: string): LogsDb {
  const dbFile = path.join(logsDir, DB_FILE_NAME);
  if (!fs.existsSync(dbFile)) {
    throw new Error(
      `logging database not found at ${dbFile}. ` +
        `Start rhd_ai_proxy with proxy.logging enabled (logging.path = ${logsDir}) ` +
        `so the database is created, then restart this dev server.`
    );
  }
  const raw = new Database(dbFile);
  raw.pragma(`busy_timeout = ${BUSY_TIMEOUT_MS}`);
  return { raw };
}

export function closeLogsDb(db: LogsDb): void {
  db.raw.close();
}
```

### 3. `frontend-proxy/src/server/queries.ts` — SELECT-only data access

```typescript
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
```

### 4. `frontend-proxy/src/server/routes.ts` — connect middleware

```typescript
import type { Connect, IncomingMessage, ServerResponse } from 'vite';
import type { ZodType } from 'zod';
import { ChatsResponseSchema, ChatDetailSchema, RequestDetailSchema } from '../lib/api/schemas.js';
import type { LogsDb } from './db.js';
import { getChatDetail, getRequestDetail, listChats } from './queries.js';

/**
 * Pure route matcher, separated for unit testing.
 * Returns the extracted numeric id for detail routes, null for /api/chats, and
 * undefined when the path matches no route.
 */
export function matchApiRoute(
  pathname: string
): { route: 'chats' } | { route: 'chat-detail'; chatId: number } |
    { route: 'request-detail'; requestId: number } | undefined {
  if (pathname === '/api/chats') {
    return { route: 'chats' };
  }
  let match = /^\/api\/chats\/(\d+)$/.exec(pathname);
  if (match) {
    return { route: 'chat-detail', chatId: Number(match[1]) };
  }
  match = /^\/api\/requests\/(\d+)$/.exec(pathname);
  if (match) {
    return { route: 'request-detail', requestId: Number(match[1]) };
  }
  return undefined;
}

function sendJson(res: ServerResponse, status: number, body: unknown): void {
  res.statusCode = status;
  res.setHeader('content-type', 'application/json; charset=utf-8');
  res.end(JSON.stringify(body));
}

function sendError(res: ServerResponse, status: number, message: string): void {
  sendJson(res, status, { error: message });
}

/**
 * Connect middleware serving the read-only proxy-logs API.
 * Only GET is supported; every failure is a JSON error envelope.
 */
export function createApiMiddleware(db: LogsDb): Connect.NextHandleFunction {
  return (req: IncomingMessage, res: ServerResponse, next) => {
    const url = new URL(req.url ?? '/', 'http://localhost');
    if (!url.pathname.startsWith('/api/')) {
      next();
      return;
    }

    if (req.method !== 'GET') {
      sendError(res, 405, `method ${req.method} not supported (read-only API)`);
      return;
    }

    const matched = matchApiRoute(url.pathname);
    if (!matched) {
      sendError(res, 404, `unknown API route: ${url.pathname}`);
      return;
    }

    try {
      // Final schemas validate what we hand out matches the contract.
      switch (matched.route) {
        case 'chats':
          sendJson(res, 200, ChatsResponseSchema.parse({ chats: listChats(db.raw) }));
          break;
        case 'chat-detail': {
          const detail = getChatDetail(db.raw, matched.chatId);
          if (detail === null) {
            sendError(res, 404, `chat ${matched.chatId} not found`);
          } else {
            sendJson(res, 200, ChatDetailSchema.parse(detail));
          }
          break;
        }
        case 'request-detail': {
          const detail = getRequestDetail(db.raw, matched.requestId);
          if (detail === null) {
            sendError(res, 404, `request ${matched.requestId} not found`);
          } else {
            sendJson(res, 200, RequestDetailSchema.parse(detail));
          }
          break;
        }
      }
    } catch (error) {
      sendError(
        res,
        500,
        error instanceof Error ? error.message : 'internal server error'
      );
    }
  };
}
```

(`ZodType` import is unused in the final code above — drop it from the import list.)

### 5. `frontend-proxy/src/server/plugin.ts` — the Vite plugin

```typescript
import type { Plugin } from 'vite';
import { openLogsDb } from './db.js';
import { createApiMiddleware } from './routes.js';

/** Environment variable pointing at the directory that contains chats.sqlite3. */
export const PROXY_LOGS_PATH_ENV = 'VITE_PROXY_LOGS_PATH';

/**
 * Resolve and validate the logging directory from the environment.
 * Throws a descriptive error when unset — called from vite.config.ts so the
 * dev server fails fast at config load.
 */
export function resolveLogsDir(env: NodeJS.ProcessEnv = process.env): string {
  const value = env[PROXY_LOGS_PATH_ENV];
  if (!value || value.trim() === '') {
    throw new Error(
      `${PROXY_LOGS_PATH_ENV} is not set. It must point at the directory that ` +
        `contains rhd_ai_proxy's chats.sqlite3 logging database — the same folder ` +
        `configured as proxy.logging.path in the proxy YAML. Example: ` +
        `${PROXY_LOGS_PATH_ENV}=./packages/rhd_ai_proxy/proxy-logs npm run dev`
    );
  }
  return value;
}

/**
 * Vite plugin exposing the read-only proxy-logs JSON API on the dev server.
 * The database is opened once when the server starts.
 */
export function proxyLogsApiPlugin(logsDir: string): Plugin {
  return {
    name: 'rhd-proxy-logs-api',
    configureServer(server) {
      const db = openLogsDb(logsDir);
      server.middlewares.use(createApiMiddleware(db));
    }
  };
}
```

### 6. `frontend-proxy/src/server/fixture.ts` — test fixture (Node only)

Builds temporary logging databases for tests. The schema DDL is **copied deliberately** from
`packages/rhd_ai_proxy/src/logging/db.rs` (`SCHEMA` constant): the viewer must not import Rust
code, and the schema is frozen (CREATE TABLE IF NOT EXISTS). Note the provenance in a comment so
future schema changes in the proxy update this file too.

```typescript
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import Database from 'better-sqlite3';
import { DB_FILE_NAME } from './db.js';

// COPIED from packages/rhd_ai_proxy/src/logging/db.rs (SCHEMA). Keep in sync.
export const LOGGING_SCHEMA = `
CREATE TABLE IF NOT EXISTS chats (
    id         INTEGER PRIMARY KEY,
    title      TEXT NOT NULL,
    model      TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS prefix_hashes (
    hash    BLOB PRIMARY KEY,
    chat_id INTEGER NOT NULL REFERENCES chats(id)
);
CREATE TABLE IF NOT EXISTS requests (
    id                 INTEGER PRIMARY KEY,
    chat_id            INTEGER NOT NULL REFERENCES chats(id),
    ts                 TEXT NOT NULL,
    method             TEXT NOT NULL,
    path               TEXT NOT NULL,
    model              TEXT,
    stream             INTEGER NOT NULL DEFAULT 0,
    status             INTEGER,
    duration_ms        INTEGER,
    error              TEXT,
    response_assembled TEXT
);
CREATE TABLE IF NOT EXISTS raw (
    request_id    INTEGER PRIMARY KEY REFERENCES requests(id),
    request_body  BLOB NOT NULL,
    response_body BLOB
);
CREATE INDEX IF NOT EXISTS idx_requests_chat ON requests(chat_id, id);
`;

export interface FixtureDb {
  db: Database.Database;
  /** The directory containing chats.sqlite3 (use as VITE_PROXY_LOGS_PATH). */
  dir: string;
  cleanup: () => void;
}

export function createFixtureDb(): FixtureDb {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'frontend-proxy-logs-'));
  const db = new Database(path.join(dir, DB_FILE_NAME));
  db.exec(LOGGING_SCHEMA);
  return {
    db,
    dir,
    cleanup: () => {
      db.close();
      fs.rmSync(dir, { recursive: true, force: true });
    }
  };
}

export interface SeedRequest {
  chatId: number;
  ts?: string;
  model?: string | null;
  stream?: boolean;
  status?: number | null;
  durationMs?: number | null;
  error?: string | null;
  responseAssembled?: string | null;
  requestBody: string;
  responseBody?: string | null;
}

/** Insert one chat; returns its id. */
export function seedChat(db: Database.Database, title: string, model: string | null = null): number {
  const now = new Date().toISOString();
  const info = db.prepare(
    'INSERT INTO chats (title, model, created_at, updated_at) VALUES (?, ?, ?, ?)'
  ).run(title, model, now, now);
  return Number(info.lastInsertRowid);
}

/** Insert one request + raw row; returns the request id. */
export function seedRequest(db: Database.Database, seed: SeedRequest): number {
  const info = db.prepare(
    `INSERT INTO requests (chat_id, ts, method, path, model, stream, status, duration_ms, error, response_assembled)
     VALUES (?, ?, 'POST', '/v1/chat/completions', ?, ?, ?, ?, ?, ?)`
  ).run(
    seed.chatId,
    seed.ts ?? new Date().toISOString(),
    seed.model ?? null,
    seed.stream ? 1 : 0,
    seed.status ?? null,
    seed.durationMs ?? null,
    seed.error ?? null,
    seed.responseAssembled ?? null
  );
  const requestId = Number(info.lastInsertRowid);
  db.prepare('INSERT INTO raw (request_id, request_body, response_body) VALUES (?, ?, ?)').run(
    requestId,
    Buffer.from(seed.requestBody, 'utf-8'),
    seed.responseBody === undefined || seed.responseBody === null
      ? null
      : Buffer.from(seed.responseBody, 'utf-8')
  );
  return requestId;
}
```

### 7. `frontend-proxy/src/server/server.test.ts` — server tests

Node-environment test file (**must start with the docblock**) covering queries, conversation
derivation, route matching, the middleware over real HTTP, and `openLogsDb`/`resolveLogsDir`
fail-fast behavior:

```typescript
// @vitest-environment node
import { afterEach, describe, expect, it } from 'vitest';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import { createFixtureDb, seedChat, seedRequest, type FixtureDb } from './fixture.js';
import { matchApiRoute } from './routes.js';
import { openLogsDb } from './db.js';
import { buildConversation, getChatDetail, getRequestDetail, listChats, listRequests } from './queries.js';
import { resolveLogsDir } from './plugin.js';
// createApiMiddleware imported for the HTTP-level test
```

**Test cases (write all of these):**

1. `listChats` — seed two chats where the older-created one has the newer `updated_at`; assert
   order is `updated_at DESC` and `requestCount` matches seeded requests per chat.
2. `listRequests` — seed a chat with 3 requests (completed 200, in-flight `status NULL`, errored
   with `error: 'upstream unreachable'`); assert oldest-first order, `stream` boolean mapping,
   `status`/`durationMs`/`error` nullability, `hasAssembled` true only for the row with
   `responseAssembled`.
3. `listRequests` — unknown chat id returns `null`.
4. `getChatDetail` — happy path returns `{ chat, requests, conversation }` with `chat.requestCount`
   consistent; unknown id returns `null`.
5. `buildConversation` — append-only chat: seed requests whose `requestBody` JSON embeds the
   growing history (`[system, u1]`, then `[system, u1, a1, u2]`); assert conversation = latest
   history mapped to `message` turns (roles/content preserved) **plus exactly one** `assistant`
   turn for the latest request id with the seeded `responseAssembled` string.
6. `buildConversation` — in-flight latest request (`status NULL`, `error NULL`, no assembled):
   assistant turn has `pending: true`, `content: null`, `error: null`.
7. `buildConversation` — errored latest request: assistant turn carries the `error` string and
   `pending: false`.
8. `buildConversation` — latest `requestBody` is not JSON (e.g. `'<binary>'`): conversation
   contains only the assistant turn.
9. `buildConversation` — chat with no requests returns `[]`.
10. `getRequestDetail` — streaming request seeded with SSE text as `responseBody`
    (`data: {"choices":[...]}\n\ndata: [DONE]`) and JSON `requestBody`; assert UTF-8 text round
    trip, `responseAssembled` string, and summary fields. In-flight request → `responseBody` is
    `null`. Unknown id → `null`.
11. `matchApiRoute` — `/api/chats` → chats; `/api/chats/42` → chat-detail 42;
    `/api/requests/7` → request-detail 7; `/api/chats/abc`, `/api/other`, `/api/chats/1/requests`
    → `undefined`.
12. Middleware over HTTP: build `http.createServer(createApiMiddleware({ raw: fixture.db }))`,
    listen on port 0, then `fetch`:
    - `GET /api/chats` → 200, body validates against `ChatsResponseSchema`;
    - `GET /api/chats/<unknown>` → 404 with `{ error: ... }`;
    - `GET /api/requests/<seeded>` → 200 valid `RequestDetailSchema`;
    - `POST /api/chats` → 405;
    - `GET /api/unknown` under `/api/` prefix → 404;
    - non-`/api` path → `next()` called (assert a sentinel flag middleware after it runs).
    Close the server in `afterEach`.
13. `openLogsDb` — missing file: `expect(() => openLogsDb(tmpEmptyDir)).toThrow(/not found/)` with
    the directory path in the message; existing file opens and closes cleanly.
14. `resolveLogsDir` — `{}` env → throws naming `VITE_PROXY_LOGS_PATH`; whitespace-only value →
    throws; set value → returned verbatim.

### 8. `frontend-proxy/src/server/index.ts` — barrel (optional but recommended)

Re-export the plugin surface so `vite.config.ts` imports from one place:

```typescript
export { PROXY_LOGS_PATH_ENV, proxyLogsApiPlugin, resolveLogsDir } from './plugin.js';
export { DB_FILE_NAME, openLogsDb } from './db.js';
```

## Files to Modify

### 9. `frontend-proxy/vite.config.ts`

Wire env validation (fail fast at config load) and the plugin:

```typescript
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { proxyLogsApiPlugin, resolveLogsDir } from './src/server/index.js';

// Fail fast: throws when VITE_PROXY_LOGS_PATH is unset — vite exits with this message.
const logsDir = resolveLogsDir();

export default defineConfig({
  plugins: [svelte(), proxyLogsApiPlugin(logsDir)],
  server: {
    port: 5174,
    strictPort: true
  },
  css: {
    modules: {
      localsConvention: 'camelCase'
    }
  }
});
```

Vite loads the config with esbuild (TypeScript imports from `src/` work) and any throw aborts
startup with the message printed — that is the fail-fast mechanism.

### 10. `frontend-proxy/package.json`

Add to `devDependencies`:

```json
"@types/better-sqlite3": "^7.6.13",
"better-sqlite3": "^13.0.3"
```

(they are dev-only because the middleware runs in the dev server, never in shipped client code;
placing them in `dependencies` is also acceptable — pick devDependencies and note it.)

Then run `npm install` in `frontend-proxy/`.

## API Examples

`GET /api/chats` →

```json
{ "chats": [ { "id": 7, "title": "hello", "model": "z-ai/glm-5.3",
  "createdAt": "2026-09-27T10:00:00+00:00", "updatedAt": "2026-09-27T10:05:00+00:00",
  "requestCount": 3 } ] }
```

`GET /api/chats/7` →

```json
{ "chat": { "...": "as above" },
  "requests": [ { "id": 21, "chatId": 7, "ts": "2026-09-27T10:00:01+00:00", "method": "POST",
    "path": "/v1/chat/completions", "model": "z-ai/glm-5.3", "stream": true, "status": 200,
    "durationMs": 1234, "error": null, "hasAssembled": true } ],
  "conversation": [
    { "kind": "message", "role": "system", "content": "You are helpful." },
    { "kind": "message", "role": "user", "content": "hello" },
    { "kind": "assistant", "requestId": 21, "content": "{\"content\":\"Hi!\"}",
      "error": null, "pending": false } ] }
```

`GET /api/requests/21` →

```json
{ "summary": { "id": 21, "...": "as above" },
  "requestBody": "{\"model\":\"z-ai/glm-5.3\",\"messages\":[...]}",
  "responseBody": "data: {\"choices\":[...]}\n\ndata: [DONE]\n\n",
  "responseAssembled": "{\"content\":\"Hi!\"}" }
```

## Implementation Notes

1. **Why normal open instead of `readonly: true` (AD-4):** SQLite cannot open a WAL database
   read-only when the `-shm`/`-wal` pair needs recovery and no writer is attached — the proxy is
   often not running while you inspect logs. Read-only is guaranteed by queries.ts containing
   only SELECTs; the middleware additionally rejects non-GET with 405.
2. **Zod `.parse` on the way out:** validating our own responses costs microseconds on a debug
   dataset and turns contract drift into an immediate 500 with a message instead of a client-side
   mystery. It also keeps the schemas executable documentation.
3. **Conversation duplication pitfall:** do NOT interleave every request's `responseAssembled`
   into the spine — append-only harnesses (the common case, incl. rhd itself) already contain
   prior assistant turns in each request's history, so interleaving would duplicate them. Only
   the latest request's outcome becomes the trailing assistant turn (see `buildConversation`).
4. **`Uint8Array` typing:** better-sqlite3 returns BLOBs as `Uint8Array`; map through
   `Buffer.from(...).toString('utf-8')`. Lossy on invalid UTF-8 — acceptable for a debug viewer.
5. **Env validation placement:** in `vite.config.ts` (not inside `configureServer`) so the error
   surfaces as a config-load failure with vite's clean error rendering before any port binding.
6. **`vite build` requires the env var too** — accepted consequence of validating in the config
   (dev-only tool; see grand plan scope decision). Alternative (validate only in
   `configureServer`) was rejected to keep the failure as early as possible.
7. **Fixture schema sync:** `LOGGING_SCHEMA` must match the proxy's `SCHEMA`. If
   `rhd_ai_proxy` ever migrates its logging schema, update `fixture.ts` in the same change.
8. **Single shared connection:** the middleware opens one connection per dev-server process;
   better-sqlite3 is synchronous, so no async glue is needed inside connect handlers.

## Dependencies

- **Depends on:** Phase 1 (scaffold, vitest config, mise tasks, `@types/node` already present).
- **Blocks:** Phase 3 consumes `src/lib/api/schemas.ts` and the live endpoints; Phases 4–5 depend
  on Phase 3 transitively. Phase 6 documents the env var and endpoints.
- **Parallel with:** nothing — the schemas created here are the contract for everything after.

## Verification Summary

```sh
cd frontend-proxy && npm install            # pulls better-sqlite3
npm run test                                # server.test.ts (node env) all green
npm run check                               # svelte-check incl. src/server
# without env var — must fail with a message naming VITE_PROXY_LOGS_PATH:
npm run dev && echo "should not reach here"
# with env var:
VITE_PROXY_LOGS_PATH=<proxy-logs-dir> npm run dev
curl -s localhost:5174/api/chats | head -c 400
curl -s localhost:5174/api/chats/1 | head -c 400
curl -s localhost:5174/api/requests/1 | head -c 400
curl -s -o /dev/null -w '%{http_code}\n' localhost:5174/api/chats/999   # 404
```
