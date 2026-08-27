import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import ConnectionStatusHarness from './ConnectionStatusHarness.svelte';
import { ConnectionStore } from '../../stores/ConnectionStore.js';
import type { AppStore } from '../../stores/AppStore.js';
import { websocket } from '../api/websocket.js';

/**
 * Minimal AppStore-shaped object used for component tests.
 * Only the `connection` substore is accessed by ConnectionStatus.
 * Cast to AppStore because the harness prop type is AppStore.
 */
function createMockAppStore(): AppStore {
  return {
    connection: new ConnectionStore()
    // Other AppStore members are not accessed by ConnectionStatus.
  } as AppStore;
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  // Ensure the websocket singleton has no leftover connection for other tests.
  websocket.disconnect();
});

describe('ConnectionStatus', () => {
  beforeEach(() => {
    // The component imports the websocket singleton for handleReconnect.
    // Stub the global WebSocket so connecting does not hit a real socket.
    const MockWebSocketImpl = class MockWebSocket {
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

      constructor(_url: string) {}
    };
    vi.stubGlobal('WebSocket', MockWebSocketImpl);
  });

  it('should render disconnected status with reconnect button', () => {
    const appStore = createMockAppStore();
    appStore.connection.setDisconnected('Connection lost');

    const { getByText } = render(ConnectionStatusHarness, { props: { appStore } });

    expect(getByText('Disconnected')).toBeTruthy();
    expect(getByText('Reconnect')).toBeTruthy();
  });

  it('should render connecting status', () => {
    const appStore = createMockAppStore();
    appStore.connection.setConnecting();

    const { getByText } = render(ConnectionStatusHarness, { props: { appStore } });

    expect(getByText('Connecting...')).toBeTruthy();
  });

  it('should render nothing when connected', () => {
    const appStore = createMockAppStore();
    appStore.connection.setConnected();

    const { queryByText } = render(ConnectionStatusHarness, { props: { appStore } });

    expect(queryByText('Connected')).toBeNull();
    expect(queryByText('Reconnect')).toBeNull();
  });

  it('should call websocket.disconnect and connect on reconnect click', async () => {
    const appStore = createMockAppStore();
    appStore.connection.setDisconnected('Connection lost');

    const disconnectSpy = vi.spyOn(websocket, 'disconnect');
    const connectSpy = vi.spyOn(websocket, 'connect');

    const { getByText } = render(ConnectionStatusHarness, { props: { appStore } });

    await fireEvent.click(getByText('Reconnect'));

    expect(disconnectSpy).toHaveBeenCalled();

    // handleReconnect awaits a 100ms delay before calling connect().
    await new Promise((resolve) => setTimeout(resolve, 150));
    expect(connectSpy).toHaveBeenCalled();

    disconnectSpy.mockRestore();
    connectSpy.mockRestore();
  });
});