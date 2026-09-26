// @vitest-environment node
import { afterEach, describe, expect, it } from 'vitest';
import http from 'node:http';
import type { AddressInfo } from 'node:net';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import type Database from 'better-sqlite3';
import {
  ChatsResponseSchema,
  ChatDetailSchema,
  RequestDetailSchema,
  ApiErrorSchema
} from '../lib/api/schemas.js';
import { createFixtureDb, seedChat, seedRequest, type FixtureDb } from './fixture.js';
import { createApiMiddleware, matchApiRoute } from './routes.js';
import { closeLogsDb, openLogsDb } from './db.js';
import {
  buildConversation,
  getChatDetail,
  getRequestDetail,
  listChats,
  listRequests
} from './queries.js';
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

/** Overwrite a chat's timestamps (seedChat always uses "now"). */
function setChatTimestamps(
  db: Database.Database,
  chatId: number,
  createdAt: string,
  updatedAt: string
): void {
  db.prepare('UPDATE chats SET created_at = ?, updated_at = ? WHERE id = ?')
    .run(createdAt, updatedAt, chatId);
}

describe('listChats', () => {
  it('orders by updated_at desc and reports per-chat request counts', () => {
    const fixture = newFixture();
    // Older-created chat is more recently active; newer-created chat is stale.
    const activeId = seedChat(fixture.db, 'older but active', 'z-ai/glm-5.3');
    const staleId = seedChat(fixture.db, 'newer but stale', null);
    setChatTimestamps(fixture.db, activeId, '2026-01-01T00:00:00+00:00', '2026-03-01T00:00:00+00:00');
    setChatTimestamps(fixture.db, staleId, '2026-02-01T00:00:00+00:00', '2026-02-15T00:00:00+00:00');    seedRequest(fixture.db, {
      chatId: activeId,
      status: 200,
      requestBody: '{"messages":[]}'
    });
    seedRequest(fixture.db, {
      chatId: activeId,
      status: 200,
      requestBody: '{"messages":[]}'
    });
    seedRequest(fixture.db, {
      chatId: staleId,
      status: 200,
      requestBody: '{"messages":[]}'
    });

    const chats = listChats(fixture.db);
    expect(chats.map((chat) => chat.id)).toEqual([activeId, staleId]);
    expect(chats[0]).toMatchObject({
      title: 'older but active',
      model: 'z-ai/glm-5.3',
      createdAt: '2026-01-01T00:00:00+00:00',
      updatedAt: '2026-03-01T00:00:00+00:00',
      requestCount: 2
    });
    expect(chats[1]).toMatchObject({ title: 'newer but stale', model: null, requestCount: 1 });
  });
});

describe('listRequests', () => {
  it('returns summaries oldest-first with mapped booleans and nullability', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'mixed outcomes');
    const completedId = seedRequest(fixture.db, {
      chatId,
      ts: '2026-09-27T10:00:01+00:00',
      model: 'z-ai/glm-5.3',
      stream: true,
      status: 200,
      durationMs: 1234,
      responseAssembled: '{"role":"assistant","content":"Hi!"}',
      requestBody: '{"messages":[]}',
      responseBody: 'data: {"choices":[]}\n\ndata: [DONE]\n\n'
    });
    const inFlightId = seedRequest(fixture.db, {
      chatId,
      ts: '2026-09-27T10:00:05+00:00',
      model: 'z-ai/glm-5.3',
      requestBody: '{"messages":[]}'
    });
    const erroredId = seedRequest(fixture.db, {
      chatId,
      ts: '2026-09-27T10:00:09+00:00',
      model: 'z-ai/glm-5.3',
      status: 502,
      durationMs: 500,
      error: 'upstream unreachable',
      requestBody: '{"messages":[]}'
    });

    const requests = listRequests(fixture.db, chatId)!;
    expect(requests.map((request) => request.id)).toEqual([completedId, inFlightId, erroredId]);

    expect(requests[0]).toEqual({
      id: completedId,
      chatId,
      ts: '2026-09-27T10:00:01+00:00',
      method: 'POST',
      path: '/v1/chat/completions',
      model: 'z-ai/glm-5.3',
      stream: true,
      status: 200,
      durationMs: 1234,
      error: null,
      hasAssembled: true
    });

    expect(requests[1]).toMatchObject({
      id: inFlightId,
      stream: false,
      status: null,
      durationMs: null,
      error: null,
      hasAssembled: false
    });
    expect(requests[2]).toMatchObject({
      id: erroredId,
      status: 502,
      error: 'upstream unreachable',
      hasAssembled: false
    });
  });

  it('returns null for an unknown chat id', () => {
    const fixture = newFixture();
    expect(listRequests(fixture.db, 999)).toBeNull();
  });
});

describe('getChatDetail', () => {
  it('returns summary, requests and conversation for a known chat', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'detail chat', 'z-ai/glm-5.3');
    seedRequest(fixture.db, {
      chatId,
      status: 200,
      durationMs: 10,
      responseAssembled: '{"role":"assistant"}',
      requestBody: JSON.stringify({
        model: 'z-ai/glm-5.3',
        messages: [{ role: 'system', content: 'You are helpful.' }, { role: 'user', content: 'hello' }]
      }),
      responseBody: '{"ok":true}'
    });

    const detail = getChatDetail(fixture.db, chatId)!;
    expect(detail.chat.id).toBe(chatId);
    expect(detail.chat.requestCount).toBe(1);
    expect(detail.requests).toHaveLength(1);
    expect(detail.conversation.map((turn) => turn.kind)).toEqual(['message', 'message', 'assistant']);
  });

  it('returns null for an unknown chat id', () => {
    const fixture = newFixture();
    expect(getChatDetail(fixture.db, 4242)).toBeNull();
  });
});

describe('buildConversation', () => {
  it('derives the spine from the latest request history plus one assistant turn', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'append-only');
    seedRequest(fixture.db, {
      chatId,
      status: 200,
      durationMs: 100,
      responseAssembled: '{"role":"assistant","content":"a1"}',
      requestBody: JSON.stringify({
        messages: [
          { role: 'system', content: 'sys' },
          { role: 'user', content: 'u1' }
        ]
      })
    });
    const latestId = seedRequest(fixture.db, {
      chatId,
      status: 200,
      durationMs: 120,
      responseAssembled: '{"role":"assistant","content":"a2"}',
      requestBody: JSON.stringify({
        messages: [
          { role: 'system', content: 'sys' },
          { role: 'user', content: 'u1' },
          { role: 'assistant', content: 'a1' },
          { role: 'user', content: 'u2' }
        ]
      })
    });

    const turns = buildConversation(fixture.db, chatId);
    expect(turns).toHaveLength(5);
    expect(turns.slice(0, 4)).toEqual([
      { kind: 'message', role: 'system', content: 'sys' },
      { kind: 'message', role: 'user', content: 'u1' },
      { kind: 'message', role: 'assistant', content: 'a1' },
      { kind: 'message', role: 'user', content: 'u2' }
    ]);
    const assistantTurns = turns.filter((turn) => turn.kind === 'assistant');
    expect(assistantTurns).toHaveLength(1);
    expect(assistantTurns[0]).toEqual({
      kind: 'assistant',
      requestId: latestId,
      content: '{"role":"assistant","content":"a2"}',
      error: null,
      pending: false
    });
  });

  it('marks an in-flight latest request as pending with null content', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'in flight');
    const requestId = seedRequest(fixture.db, {
      chatId,
      requestBody: JSON.stringify({ messages: [{ role: 'user', content: 'hi' }] })
    });

    const turns = buildConversation(fixture.db, chatId);
    expect(turns[turns.length - 1]).toEqual({
      kind: 'assistant',
      requestId,
      content: null,
      error: null,
      pending: true
    });
  });

  it('carries the error string on an errored latest request', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'errored');
    const requestId = seedRequest(fixture.db, {
      chatId,
      status: 502,
      error: 'upstream unreachable',
      requestBody: JSON.stringify({ messages: [{ role: 'user', content: 'hi' }] })
    });

    const turns = buildConversation(fixture.db, chatId);
    expect(turns[turns.length - 1]).toEqual({
      kind: 'assistant',
      requestId,
      content: null,
      error: 'upstream unreachable',
      pending: false
    });
  });

  it('degrades to the assistant turn only when the latest body is not JSON', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'binary body');
    const requestId = seedRequest(fixture.db, {
      chatId,
      status: 200,
      durationMs: 5,
      responseAssembled: '{"role":"assistant"}',
      requestBody: '<binary>'
    });

    const turns = buildConversation(fixture.db, chatId);
    expect(turns).toEqual([
      { kind: 'assistant', requestId, content: '{"role":"assistant"}', error: null, pending: false }
    ]);
  });

  it('returns an empty conversation for a chat without requests', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'empty chat');
    expect(buildConversation(fixture.db, chatId)).toEqual([]);
  });
});

describe('getRequestDetail', () => {
  it('round-trips SSE and JSON bodies as UTF-8 text', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'raw bodies');
    const requestBody = JSON.stringify({
      model: 'z-ai/glm-5.3',
      messages: [{ role: 'user', content: 'hello' }]
    });
    const responseBody = 'data: {"choices":[{"delta":{"content":"Hi"}}]}\n\ndata: [DONE]\n\n';
    const requestId = seedRequest(fixture.db, {
      chatId,
      model: 'z-ai/glm-5.3',
      stream: true,
      status: 200,
      durationMs: 999,
      responseAssembled: '{"role":"assistant","content":"Hi"}',
      requestBody,
      responseBody
    });

    const detail = getRequestDetail(fixture.db, requestId)!;
    expect(detail.summary).toMatchObject({
      id: requestId,
      chatId,
      model: 'z-ai/glm-5.3',
      stream: true,
      status: 200,
      durationMs: 999,
      hasAssembled: true
    });
    expect(detail.requestBody).toBe(requestBody);
    expect(detail.responseBody).toBe(responseBody);
    expect(detail.responseAssembled).toBe('{"role":"assistant","content":"Hi"}');
  });

  it('returns a null response body while the request is in flight', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'in flight');
    const requestId = seedRequest(fixture.db, {
      chatId,
      requestBody: '{"messages":[]}'
    });

    const detail = getRequestDetail(fixture.db, requestId)!;
    expect(detail.responseBody).toBeNull();
    expect(detail.responseAssembled).toBeNull();
  });

  it('returns null for an unknown request id', () => {
    const fixture = newFixture();
    expect(getRequestDetail(fixture.db, 9999)).toBeNull();
  });
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

  it('opens an existing database and closes it cleanly', () => {
    const fixture = newFixture();
    const db = openLogsDb(fixture.dir);
    expect(db.raw.prepare('SELECT COUNT(*) AS n FROM chats').get()).toEqual({ n: 0 });
    closeLogsDb(db);
  });
});

describe('resolveLogsDir', () => {
  it('requires a non-blank VITE_PROXY_LOGS_PATH and returns set values verbatim', () => {
    expect(() => resolveLogsDir({})).toThrow(/VITE_PROXY_LOGS_PATH/);
    expect(() => resolveLogsDir({ VITE_PROXY_LOGS_PATH: '   ' })).toThrow(/VITE_PROXY_LOGS_PATH/);
    expect(resolveLogsDir({ VITE_PROXY_LOGS_PATH: './some/dir' })).toBe('./some/dir');
  });
});
