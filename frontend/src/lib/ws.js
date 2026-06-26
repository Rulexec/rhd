import { activeScenarios, finishedScenarios, lastKnownId, wsConnected } from './stores.js';

const WS_PORT = import.meta.env.VITE_WS_PORT || 9876;
const WS_URL = `ws://127.0.0.1:${WS_PORT}`;
const RECONNECT_DELAY_MS = 2000;

let socket = null;
let requestIdCounter = 0;
const pendingRequests = new Map();
let reconnectTimer = null;

function generateRequestId() {
  requestIdCounter += 1;
  return `req-${requestIdCounter}`;
}

function connect() {
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

  socket.onmessage = (event) => {
    const message = JSON.parse(event.data);

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

function scheduleReconnect() {
  if (reconnectTimer) return;
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connect();
  }, RECONNECT_DELAY_MS);
}

function sendRequest(request) {
  return new Promise((resolve, reject) => {
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      reject(new Error('WebSocket not connected'));
      return;
    }
    pendingRequests.set(request.id, resolve);
    socket.send(JSON.stringify(request));
  });
}

function handleEvent(message) {
  const { event, data } = message;

  switch (event) {
    case 'scenariostarted':
      activeScenarios.addScenario(data);
      break;
    case 'stepstarted':
      activeScenarios.updateStep(data.executionId, data.stepName, data.startedAt);
      break;
    case 'scenariofinished':
      activeScenarios.removeScenario(data.id);
      finishedScenarios.prepend(data);
      lastKnownId.set(data.id);
      break;
  }
}

export function initWebSocket() {
  connect();
}

export async function subscribe() {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'subscribe', id });
  if (response.success && response.data?.activeExecutions) {
    activeScenarios.setFromList(response.data.activeExecutions);
  }
  return response;
}

export async function getFinishedScenarios(lastId) {
  const id = generateRequestId();
  const request = { type: 'getFinishedScenarios', id };
  if (lastId !== undefined && lastId !== null) {
    request.lastId = lastId;
  }
  const response = await sendRequest(request);
  return response;
}
