import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { websocket } from './websocket';
import { ConnectionStore } from '../../stores/ConnectionStore';

/**
 * Mock WebSocket with the static readyState constants that WebSocketClient
 * references (CONNECTING/OPEN) so connect() guard checks behave correctly.
 */
class MockWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;

  readyState: number = MockWebSocket.CONNECTING;
  onopen: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  send = vi.fn();
  close = vi.fn();

  constructor(_url: string) {
    lastMockWebSocket = this;
  }
}

let lastMockWebSocket: MockWebSocket | null = null;

describe('WebSocket with ConnectionStore', () => {
  let connectionStore: ConnectionStore;

  beforeEach(() => {
    // Ensure the singleton references no leftover socket from a previous test.
    websocket.disconnect();
    connectionStore = new ConnectionStore();
    websocket.setConnectionStore(connectionStore);
    vi.stubGlobal('WebSocket', MockWebSocket);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    lastMockWebSocket = null;
  });

  it('should drive the MobX connection store through the WebSocket lifecycle', () => {
    expect(connectionStore.status).toBe('disconnected');

    websocket.connect();
    expect(connectionStore.status).toBe('connecting');

    // Simulate connection open
    lastMockWebSocket!.readyState = MockWebSocket.OPEN;
    lastMockWebSocket!.onopen?.({} as Event);
    expect(connectionStore.status).toBe('connected');
    expect(connectionStore.isConnected).toBe(true);

    // Simulate connection close (code 1006 = lost unexpectedly)
    lastMockWebSocket!.onclose?.({ code: 1006, reason: '' } as CloseEvent);
    expect(connectionStore.status).toBe('disconnected');
    expect(connectionStore.error).toBe('Connection lost unexpectedly');
    expect(connectionStore.isConnected).toBe(false);
  });

  it('should update connection store on user disconnect', () => {
    connectionStore.setConnected();
    expect(connectionStore.isConnected).toBe(true);

    websocket.disconnect();

    expect(connectionStore.status).toBe('disconnected');
    expect(connectionStore.error).toBe(null);
    expect(connectionStore.isConnected).toBe(false);
  });
});