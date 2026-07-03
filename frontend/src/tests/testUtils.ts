let CONTROL_PORT = import.meta.env.VITE_CONTROL_PORT || 3001;
let CONTROL_URL = `http://localhost:${CONTROL_PORT}`;
let WS_PORT = import.meta.env.VITE_WS_PORT || 9876;

export function setControlPort(port: number): void {
  CONTROL_PORT = port;
  CONTROL_URL = `http://localhost:${CONTROL_PORT}`;
}

export function setWsPort(port: number): void {
  WS_PORT = port;
}

export function getWsPort(): number {
  return WS_PORT;
}

export async function configureMock(content: string): Promise<void> {
  await fetch(`${CONTROL_URL}/mock-response`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ content }),
  });
}

export async function getRecordedRequests(): Promise<any[]> {
  const response = await fetch(`${CONTROL_URL}/requests`);
  const data = await response.json();
  return data.requests;
}

async function checkControlResponse(response: Response, operation: string): Promise<void> {
  if (!response.ok) {
    throw new Error(`${operation} failed with status ${response.status}`);
  }
  const data = await response.json();
  if (data.status !== 'OK') {
    throw new Error(`${operation} failed: ${data.message || 'Unknown error'}`);
  }
}

export async function emitStreamChunk(content: string): Promise<void> {
  const response = await fetch(`${CONTROL_URL}/stream-chunk`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ content }),
  });
  await checkControlResponse(response, 'emitStreamChunk');
}

export async function finishStream(): Promise<void> {
  const response = await fetch(`${CONTROL_URL}/stream-finish`, {
    method: 'POST',
  });
  await checkControlResponse(response, 'finishStream');
}

export async function setAutoStream(enabled: boolean): Promise<void> {
  const response = await fetch(`${CONTROL_URL}/set-auto-stream`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ enabled }),
  });
  await checkControlResponse(response, 'setAutoStream');
}

export async function waitForStreamReady(timeout = 5000): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    const response = await fetch(`${CONTROL_URL}/stream-ready`);
    const data = await response.json();
    if (data.ready) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error('Stream not ready timeout');
}

export async function waitForWebSocket(timeout = 5000): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeout) {
    const response = await fetch(`${CONTROL_URL}/status`);
    const data = await response.json();
    if (data.daemon_ready) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error('WebSocket connection timeout');
}
