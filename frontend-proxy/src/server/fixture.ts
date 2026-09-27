import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import Database from 'better-sqlite3';
import { DB_FILE_NAME, EXPECTED_LOGGING_SCHEMA_VERSION } from './db.js';

// COPIED from packages/rhd_ai_proxy/src/logging/schema.rs (SCHEMA). Keep in sync.
// The viewer must not depend on the Rust crate, so the DDL is deliberately
// duplicated here. Stamp the same user_version the proxy writes so the open-time
// version guard in db.ts accepts fixture databases.
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
    chat_id INTEGER NOT NULL REFERENCES chats(id),
    len     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_prefix_hashes_chat ON prefix_hashes(chat_id, len);
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
CREATE TABLE IF NOT EXISTS messages (
    id           INTEGER PRIMARY KEY,
    chat_id      INTEGER NOT NULL REFERENCES chats(id),
    seq          INTEGER NOT NULL,
    role         TEXT NOT NULL,
    message_json TEXT NOT NULL,
    content      TEXT,
    tool_calls   TEXT,
    tool_call_id TEXT,
    name         TEXT,
    source       TEXT NOT NULL CHECK (source IN ('history', 'response')),
    request_id   INTEGER NOT NULL REFERENCES requests(id),
    UNIQUE (chat_id, seq)
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
  db.pragma(`user_version = ${EXPECTED_LOGGING_SCHEMA_VERSION}`);
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

/** One seeded message row: the full message object plus row metadata. */
export interface SeedMessage {
  /** Full OpenAI-style message object (role, content, tool_calls?, tool_call_id?, name?). */
  message: Record<string, unknown>;
  /** Whether the proxy captured it from a request history or an assembled response. */
  source?: 'history' | 'response';
}

/**
 * Insert message rows for a chat, deriving the projection columns the way the
 * proxy's extract_fields does (packages/rhd_ai_proxy/src/logging/messages.rs):
 * content as verbatim JSON text unless null, tool_calls as JSON text unless an
 * empty array. Seq continues after the chat's current MAX(seq).
 */
export function seedMessages(
  db: Database.Database,
  chatId: number,
  requestId: number,
  seeds: SeedMessage[]
): void {
  const insert = db.prepare(
    `INSERT INTO messages
       (chat_id, seq, role, message_json, content, tool_calls, tool_call_id, name, source, request_id)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
  );
  let seq = (
    db.prepare('SELECT COALESCE(MAX(seq), -1) AS max FROM messages WHERE chat_id = ?')
      .get(chatId) as { max: number }
  ).max + 1;
  for (const seed of seeds) {
    const message = seed.message;
    const role = typeof message.role === 'string' ? message.role : 'unknown';
    const content =
      message.content === undefined || message.content === null
        ? null
        : JSON.stringify(message.content);
    const toolCalls = Array.isArray(message.tool_calls) && message.tool_calls.length > 0
      ? JSON.stringify(message.tool_calls)
      : null;
    insert.run(
      chatId,
      seq,
      role,
      JSON.stringify(message),
      content,
      toolCalls,
      typeof message.tool_call_id === 'string' ? message.tool_call_id : null,
      typeof message.name === 'string' ? message.name : null,
      seed.source ?? 'history',
      requestId
    );
    seq += 1;
  }
}
