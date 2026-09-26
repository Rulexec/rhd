import type { IncomingMessage, ServerResponse } from 'node:http';
import type { Connect } from 'vite';
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
