import type { WsResponse } from '@/lib/types/ws';

type RequestHandler = (request: Record<string, unknown>) => WsResponse | Promise<WsResponse>;

const handlers = new Map<string, RequestHandler>();
let defaultHandler: RequestHandler = () => ({ type: 'response', id: '', success: true, data: {} });

export function setMockWsHandler(requestType: string, handler: RequestHandler): void {
  handlers.set(requestType, handler);
}

export function setDefaultMockWsHandler(handler: RequestHandler): void {
  defaultHandler = handler;
}

export function clearMockWsHandlers(): void {
  handlers.clear();
  defaultHandler = () => ({ type: 'response', id: '', success: true, data: {} });
}

let requestIdCounter = 0;

export function generateRequestId(): string {
  requestIdCounter += 1;
  return `mock-req-${requestIdCounter}`;
}

export async function sendRequest(request: Record<string, unknown>): Promise<WsResponse> {
  const requestType = request.type as string;
  const handler = handlers.get(requestType) || defaultHandler;
  const result = await handler(request);
  return { ...result, id: request.id as string };
}

export function connectWebSocket(): void {
  // No-op in mock
}

export function initWebSocket(): void {
  // No-op in mock
}

export function setWsPort(_port: number): void {
  // No-op in mock
}

export async function subscribe(): Promise<WsResponse> {
  return sendRequest({ type: 'subscribe', id: generateRequestId() });
}

export async function getFinishedScenarios(_lastId?: number): Promise<WsResponse> {
  return sendRequest({ type: 'getFinishedScenarios', id: generateRequestId() });
}

export async function abortScenario(_executionId: string): Promise<WsResponse> {
  return sendRequest({ type: 'abortScenario', id: generateRequestId() });
}

export async function retryScenario(_executionId: string, _model?: string): Promise<WsResponse> {
  return sendRequest({ type: 'retryScenario', id: generateRequestId() });
}

export async function abortScenarioWithError(_executionId: string): Promise<WsResponse> {
  return sendRequest({ type: 'abortScenarioWithError', id: generateRequestId() });
}

export async function devNotification(): Promise<WsResponse> {
  return sendRequest({ type: 'devNotification', id: generateRequestId() });
}
