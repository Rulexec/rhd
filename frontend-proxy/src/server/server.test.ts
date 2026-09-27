// @vitest-environment node
import { afterEach, describe, expect, it } from 'vitest';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import Database from 'better-sqlite3';
import {
  ChatsResponseSchema,
  ChatDetailSchema,
  RequestDetailSchema,
  ApiErrorSchema
} from '../lib/api/schemas.js';
import { createFixtureDb, seedChat, seedRequest, type FixtureDb } from './fixture.js';
import { createApiMiddleware, matchApiRoute } from './routes.js';
import { closeLogsDb, openLogsDb } from './db.js';
import { resolveLogsDir } from './plugin.js';

/** Fixtures created during a test, torn down in afterEach. */
const fixtures: FixtureDb[] = [];
/** HTTP servers started during a test, closed in afterEach. */
const servers: http.Server[] = [];

function newFixture(): FixtureDb {
  const fixture = createFixtureDb();
  fixtures.push(fixture);
  return fixture;
}

async function startApiServer(fixture: FixtureDb): Promise<string> {
  const middleware = createApiMiddleware({ raw: fixture.db });
  // Connect handlers require a `next` callback; the API middleware never calls
  // it for /api paths (it responds or errors), so a terminal 404 suffices.
  const server = http.createServer((req, res) => {
    middleware(req, res, () => {
      res.statusCode = 404;
      res.end();
    });
  });
  servers.push(server);
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address() as AddressInfo;
  return `http://127.0.0.1:${address.port}`;
}

afterEach(async () => {
  for (const server of servers) {
    server.closeAllConnections();
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
  servers.length = 0;
  for (const fixture of fixtures) {
    fixture.cleanup();
  }
  fixtures.length = 0;
});

describe('matchApiRoute', () => {
  it('matches the three endpoints and rejects everything else', () => {
    expect(matchApiRoute('/api/chats')).toEqual({ route: 'chats' });
    expect(matchApiRoute('/api/chats/42')).toEqual({ route: 'chat-detail', chatId: 42 });
    expect(matchApiRoute('/api/requests/7')).toEqual({ route: 'request-detail', requestId: 7 });
    expect(matchApiRoute('/api/chats/abc')).toBeUndefined();
    expect(matchApiRoute('/api/other')).toBeUndefined();
    expect(matchApiRoute('/api/chats/1/requests')).toBeUndefined();
  });
});

describe('api middleware over HTTP', () => {
  it('serves chats, chat detail and request detail with valid contract bodies', async () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'http chat', 'z-ai/glm-5.3');
    const requestId = seedRequest(fixture.db, {
      chatId,
      model: 'z-ai/glm-5.3',
      stream: true,
      status: 200,
      durationMs: 42,
      responseAssembled: '{"role":"assistant"}',
      requestBody: JSON.stringify({ messages: [{ role: 'user', content: 'hello' }] }),
      responseBody: 'data: {}\n\ndata: [DONE]\n\n'
    });
    const base = await startApiServer(fixture);

    const chatsResponse = await fetch(`${base}/api/chats`);
    expect(chatsResponse.status).toBe(200);
    const chatsBody = ChatsResponseSchema.parse(await chatsResponse.json());
    expect(chatsBody.chats.map((chat) => chat.id)).toEqual([chatId]);

    const chatResponse = await fetch(`${base}/api/chats/${chatId}`);
    expect(chatResponse.status).toBe(200);
    const chatBody = ChatDetailSchema.parse(await chatResponse.json());
    expect(chatBody.chat.id).toBe(chatId);
    expect(chatBody.requests.map((request) => request.id)).toEqual([requestId]);

    const requestResponse = await fetch(`${base}/api/requests/${requestId}`);
    expect(requestResponse.status).toBe(200);
    const requestBody = RequestDetailSchema.parse(await requestResponse.json());
    expect(requestBody.summary.id).toBe(requestId);
  });

  it('returns 404 JSON errors for unknown ids and unknown api routes', async () => {
    const fixture = newFixture();
    const base = await startApiServer(fixture);

    const chatResponse = await fetch(`${base}/api/chats/999`);
    expect(chatResponse.status).toBe(404);
    expect(ApiErrorSchema.parse(await chatResponse.json()).error).toContain('999');

    const unknownResponse = await fetch(`${base}/api/unknown`);
    expect(unknownResponse.status).toBe(404);
    expect(ApiErrorSchema.parse(await unknownResponse.json())).toBeTruthy();

    const requestResponse = await fetch(`${base}/api/requests/999`);
    expect(requestResponse.status).toBe(404);
    expect(ApiErrorSchema.parse(await requestResponse.json()).error).toContain('999');
  });

  it('rejects non-GET methods with 405', async () => {
    const fixture = newFixture();
    const base = await startApiServer(fixture);

    const response = await fetch(`${base}/api/chats`, { method: 'POST' });
    expect(response.status).toBe(405);
    expect(ApiErrorSchema.parse(await response.json()).error).toContain('read-only');
  });

  it('passes non-api paths through to the next middleware', async () => {
    const fixture = newFixture();
    let nextCalled = false;
    const server = http.createServer((req, res) => {
      createApiMiddleware({ raw: fixture.db })(req, res, () => {
        nextCalled = true;
        res.statusCode = 200;
        res.end('sentinel');
      });
    });
    servers.push(server);
    await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
    const address = server.address() as AddressInfo;
    const base = `http://127.0.0.1:${address.port}`;

    const response = await fetch(`${base}/src/App.svelte`);
    expect(response.status).toBe(200);
    expect(await response.text()).toBe('sentinel');
    expect(nextCalled).toBe(true);
  });
});

describe('openLogsDb', () => {
  it('throws a descriptive error when the database file is missing', () => {
    const emptyDir = fs.mkdtempSync(path.join(os.tmpdir(), 'frontend-proxy-missing-'));
    try {
      // openLogsDb is deterministic: both executions throw the same message.
      expect(() => openLogsDb(emptyDir)).toThrow(/not found/);
      expect(() => openLogsDb(emptyDir)).toThrow(emptyDir);
    } finally {
      fs.rmSync(emptyDir, { recursive: true, force: true });
    }
  });

  it('opens a version-stamped database and closes it cleanly', () => {
    const fixture = newFixture();
    const db = openLogsDb(fixture.dir);
    expect(db.raw.prepare('SELECT COUNT(*) AS n FROM chats').get()).toEqual({ n: 0 });
    closeLogsDb(db);
  });

  it('rejects a v1 database that predates the user_version stamp', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'frontend-proxy-v1-'));
    try {
      const dbFile = path.join(dir, 'chats.sqlite3');
      const db = new Database(dbFile);
      // Minimal v1-era shape: chats table, no user_version stamp.
      db.exec(`
        CREATE TABLE chats (
            id         INTEGER PRIMARY KEY,
            title      TEXT NOT NULL,
            model      TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
      `);
      db.close();

      expect(() => openLogsDb(dir)).toThrow(/schema version 0/);
      expect(() => openLogsDb(dir)).toThrow(dbFile);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('rejects a database stamped with a newer schema version', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'frontend-proxy-v3-'));
    try {
      const dbFile = path.join(dir, 'chats.sqlite3');
      const db = new Database(dbFile);
      db.pragma('user_version = 3');
      db.close();

      expect(() => openLogsDb(dir)).toThrow(/schema version 3/);
      expect(() => openLogsDb(dir)).toThrow(/delete or move/);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe('resolveLogsDir', () => {
  it('requires a non-blank VITE_PROXY_LOGS_PATH and returns set values verbatim', () => {
    expect(() => resolveLogsDir({})).toThrow(/VITE_PROXY_LOGS_PATH/);
    expect(() => resolveLogsDir({ VITE_PROXY_LOGS_PATH: '   ' })).toThrow(/VITE_PROXY_LOGS_PATH/);
    expect(resolveLogsDir({ VITE_PROXY_LOGS_PATH: './some/dir' })).toBe('./some/dir');
  });
});
