// @vitest-environment node
import { afterEach, describe, expect, it } from 'vitest';
import type Database from 'better-sqlite3';
import { createFixtureDb, seedChat, seedMessages, seedRequest, type FixtureDb } from './fixture.js';
import {
  buildConversation,
  getChatDetail,
  getRequestDetail,
  listChats,
  listRequests
} from './queries.js';

/** Fixtures created during a test, torn down in afterEach. */
const fixtures: FixtureDb[] = [];

function newFixture(): FixtureDb {
  const fixture = createFixtureDb();
  fixtures.push(fixture);
  return fixture;
}

afterEach(() => {
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
    setChatTimestamps(fixture.db, staleId, '2026-02-01T00:00:00+00:00', '2026-02-15T00:00:00+00:00');
    seedRequest(fixture.db, {
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
    const requestId = seedRequest(fixture.db, {
      chatId,
      status: 200,
      durationMs: 10,
      responseAssembled: '{"role":"assistant","content":"Hi"}',
      requestBody: '{"messages":[]}',
      responseBody: '{"ok":true}'
    });
    seedMessages(fixture.db, chatId, requestId, [
      { message: { role: 'system', content: 'You are helpful.' } },
      { message: { role: 'user', content: 'hello' } }
    ]);
    seedMessages(fixture.db, chatId, requestId, [
      { message: { role: 'assistant', content: 'Hi' }, source: 'response' }
    ]);

    const detail = getChatDetail(fixture.db, chatId)!;
    expect(detail.chat.id).toBe(chatId);
    expect(detail.chat.requestCount).toBe(1);
    expect(detail.requests).toHaveLength(1);
    expect(detail.conversation.map((turn) => turn.kind)).toEqual([
      'message',
      'message',
      'message'
    ]);
  });

  it('returns null for an unknown chat id', () => {
    const fixture = newFixture();
    expect(getChatDetail(fixture.db, 4242)).toBeNull();
  });
});

describe('buildConversation', () => {
  it('maps messages rows in seq order with parsed projections and no tail when completed', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'tool pipeline');
    const firstId = seedRequest(fixture.db, {
      chatId,
      status: 200,
      durationMs: 100,
      requestBody: '{"messages":[]}'
    });
    seedMessages(fixture.db, chatId, firstId, [
      { message: { role: 'system', content: 'sys' } },
      { message: { role: 'user', content: 'weather?' } }
    ]);
    seedMessages(fixture.db, chatId, firstId, [
      {
        source: 'response',
        message: {
          role: 'assistant',
          content: null,
          tool_calls: [
            {
              id: 'call_1',
              type: 'function',
              function: { name: 'get_weather', arguments: '{"city":"Oslo"}' }
            }
          ]
        }
      }
    ]);
    const secondId = seedRequest(fixture.db, {
      chatId,
      status: 200,
      durationMs: 120,
      requestBody: '{"messages":[]}'
    });
    seedMessages(fixture.db, chatId, secondId, [
      {
        message: {
          role: 'tool',
          tool_call_id: 'call_1',
          name: 'get_weather',
          content: '{"temp": 20}'
        }
      }
    ]);
    seedMessages(fixture.db, chatId, secondId, [
      { source: 'response', message: { role: 'assistant', content: 'Sunny, 20 degrees' } }
    ]);

    const turns = buildConversation(fixture.db, chatId);
    expect(turns).toHaveLength(5);
    expect(turns.every((turn) => turn.kind === 'message')).toBe(true);
    expect(turns.map((turn) => (turn as { role: string }).role)).toEqual([
      'system',
      'user',
      'assistant',
      'tool',
      'assistant'
    ]);
    expect(turns.map((turn) => (turn as { source: string }).source)).toEqual([
      'history',
      'history',
      'response',
      'history',
      'response'
    ]);
    expect(turns[0]).toEqual({
      kind: 'message',
      seq: 0,
      role: 'system',
      source: 'history',
      content: 'sys',
      toolCalls: null,
      toolCallId: null,
      name: null,
      requestId: firstId
    });
    // Tool-call turn: parsed array with function metadata.
    const toolCallTurn = turns[2] as {
      content: unknown;
      toolCalls: { function?: { name?: string } }[];
    };
    expect(toolCallTurn.content).toBeNull();
    expect(toolCallTurn.toolCalls).toHaveLength(1);
    expect(toolCallTurn.toolCalls?.[0]?.function?.name).toBe('get_weather');
    // Tool-result turn: string content as delivered, ids projected.
    expect(turns[3]).toMatchObject({
      role: 'tool',
      toolCallId: 'call_1',
      name: 'get_weather',
      content: '{"temp": 20}'
    });
    expect(turns[4]).toMatchObject({
      role: 'assistant',
      content: 'Sunny, 20 degrees',
      requestId: secondId
    });
  });

  it('renders array content parts as parsed values', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'parts');
    const requestId = seedRequest(fixture.db, {
      chatId,
      status: 200,
      requestBody: '{"messages":[]}'
    });
    seedMessages(fixture.db, chatId, requestId, [
      { message: { role: 'user', content: [{ type: 'text', text: 'multi-part hello' }] } }
    ]);

    const turns = buildConversation(fixture.db, chatId);
    expect(turns[0]).toMatchObject({
      role: 'user',
      content: [{ type: 'text', text: 'multi-part hello' }]
    });
  });

  it('appends a pending tail when the latest request is in flight', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'in flight');
    const requestId = seedRequest(fixture.db, {
      chatId,
      requestBody: '{"messages":[]}'
    });
    seedMessages(fixture.db, chatId, requestId, [
      { message: { role: 'user', content: 'hi' } }
    ]);

    const turns = buildConversation(fixture.db, chatId);
    expect(turns).toHaveLength(2);
    expect(turns[turns.length - 1]).toEqual({ kind: 'pending', requestId });
  });

  it('appends an error tail when the latest request failed', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'errored');
    const requestId = seedRequest(fixture.db, {
      chatId,
      status: 502,
      error: 'upstream unreachable',
      requestBody: '{"messages":[]}'
    });

    const turns = buildConversation(fixture.db, chatId);
    expect(turns[turns.length - 1]).toEqual({
      kind: 'error',
      requestId,
      error: 'upstream unreachable'
    });
  });

  it('adds no tail for a completed latest request without a response row', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'no assembly');
    const requestId = seedRequest(fixture.db, {
      chatId,
      status: 200,
      durationMs: 5,
      // Completed but response_assembled stayed NULL (unparseable upstream body).
      requestBody: '{"messages":[]}'
    });
    seedMessages(fixture.db, chatId, requestId, [
      { message: { role: 'user', content: 'hi' } }
    ]);

    const turns = buildConversation(fixture.db, chatId);
    expect(turns).toHaveLength(1);
    expect(turns[0]).toMatchObject({ role: 'user' });
  });

  it('passes corrupt JSON columns through as raw text instead of throwing', () => {
    const fixture = newFixture();
    const chatId = seedChat(fixture.db, 'corrupt');
    const requestId = seedRequest(fixture.db, {
      chatId,
      status: 200,
      requestBody: '{"messages":[]}'
    });
    seedMessages(fixture.db, chatId, requestId, [
      { message: { role: 'user', content: 'fine' } }
    ]);
    // Simulate a row the proxy could never write (defensive path only).
    fixture.db
      .prepare(
        `INSERT INTO messages
           (chat_id, seq, role, message_json, content, tool_calls, source, request_id)
         VALUES (?, 1, 'assistant', '{}', 'not json', 'also not json', 'response', ?)`
      )
      .run(chatId, requestId);

    const turns = buildConversation(fixture.db, chatId);
    expect(turns[1]!).toMatchObject({ role: 'assistant', content: 'not json' });
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
