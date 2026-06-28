import { activeScenarios, finishedScenarios, lastKnownId, wsConnected } from './stores';
import { WsMessageSchema } from './types/ws';
import type { WsEvent, WsResponse } from './types/ws';
import { handleChatEvent } from './chatWs';

const WS_PORT = import.meta.env.VITE_WS_PORT || 9876;
const WS_URL = `ws://127.0.0.1:${WS_PORT}`;
const RECONNECT_DELAY_MS = 2000;

let socket: WebSocket | null = null;
let requestIdCounter = 0;
const pendingRequests = new Map<string, (message: WsResponse) => void>();
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

export function generateRequestId(): string {
  requestIdCounter += 1;
  return `req-${requestIdCounter}`;
}

function connect(): void {
  if (socket && socket.readyState === WebSocket.OPEN) return;

  socket = new WebSocket(WS_URL);

  socket.onopen = () => {
    wsConnected.set(true);
  };

  socket.onclose = () => {
    wsConnected.set(false);
    socket = null;
    scheduleReconnect();
  };

  socket.onerror = () => {
    socket?.close();
  };

  socket.onmessage = (event: MessageEvent) => {
    const raw = JSON.parse(event.data);
    const result = WsMessageSchema.safeParse(raw);

    if (!result.success) {
      console.error('Invalid WebSocket message:', result.error);
      return;
    }

    const message = result.data;

    if (message.type === 'response') {
      const resolver = pendingRequests.get(message.id);
      if (resolver) {
        pendingRequests.delete(message.id);
        resolver(message);
      }
    } else if (message.type === 'event') {
      handleEvent(message);
    }
  };
}

function scheduleReconnect(): void {
  if (reconnectTimer) return;
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connect();
  }, RECONNECT_DELAY_MS);
}

export function sendRequest(request: Record<string, unknown>): Promise<WsResponse> {
  return new Promise((resolve, reject) => {
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      reject(new Error('WebSocket not connected'));
      return;
    }
    pendingRequests.set(request.id as string, resolve);
    socket.send(JSON.stringify(request));
  });
}

function handleEvent(message: WsEvent): void {
  const { event, data } = message;

  if (event.startsWith('chat')) {
    handleChatEvent(event, data);
    return;
  }

  switch (event) {
    case 'scenariostarted':
      activeScenarios.addScenario(data);
      break;
    case 'stepstarted':
      activeScenarios.updateStep(data.executionId, data.stepName, data.startedAt);
      break;
    case 'scenariofinished':
      activeScenarios.removeScenario(String(data.id));
      finishedScenarios.prepend(data);
      lastKnownId.set(data.id);
      break;
  }
}

export function initWebSocket(): void {
  connect();
}

export async function subscribe(): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'subscribe', id });
  if (response.success && response.data?.activeExecutions) {
    activeScenarios.setFromList(response.data.activeExecutions);
  }
  return response;
}

export async function getFinishedScenarios(lastId?: number): Promise<WsResponse> {
  const id = generateRequestId();
  const request: Record<string, unknown> = { type: 'getFinishedScenarios', id };
  if (lastId !== undefined && lastId !== null) {
    request.lastId = lastId;
  }
  const response = await sendRequest(request);
  return response;
}

export async function abortScenario(executionId: string): Promise<WsResponse> {
  const id = generateRequestId();
  const request = { type: 'abortScenario', id, executionId };
  const response = await sendRequest(request);
  return response;
}
