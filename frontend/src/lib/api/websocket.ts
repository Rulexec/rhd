import { writable, type Writable } from 'svelte/store';
import {
  WebSocketMessageSchema,
  type WebSocketMessage,
  type Response,
  type Event
} from './schemas.js';

const WS_URL = 'ws://localhost:8080';

export type ConnectionStatus = 'connected' | 'disconnected' | 'connecting';

export interface ConnectionState {
  status: ConnectionStatus;
  error: string | null;
}

interface PendingRequest {
  resolve: (value: unknown) => void;
  reject: (reason: Error) => void;
}

type EventCallback = (data: unknown) => void;

/**
 * WebSocket client for chat server communication.
 * Handles connection lifecycle, request/response correlation, and event dispatch.
 */
class WebSocketClient {
  private ws: WebSocket | null = null;
  private pendingRequests: Map<string, PendingRequest> = new Map();
  private eventListeners: Map<string, Set<EventCallback>> = new Map();
  public isConnected: boolean = false;
  public errorMessage: string | null = null;

  /**
   * Connect to the WebSocket server.
   */
  connect(): void {
    if (this.ws && (this.ws.readyState === WebSocket.CONNECTING || this.ws.readyState === WebSocket.OPEN)) {
      console.warn('WebSocket already connected or connecting');
      return;
    }

    this.errorMessage = null;
    connectionStore.set({ status: 'connecting', error: null });

    try {
      this.ws = new WebSocket(WS_URL);

      this.ws.onopen = () => {
        console.log('WebSocket connected');
        this.isConnected = true;
        this.errorMessage = null;
        connectionStore.set({ status: 'connected', error: null });
      };

      this.ws.onclose = (event) => {
        console.log('WebSocket disconnected:', event.code, event.reason);
        this.isConnected = false;
        this.ws = null;
        this.rejectAllPending('WebSocket connection closed');

        // Provide more specific error message based on close code
        let errorMessage = 'Connection closed';
        if (event.code === 1006) {
          errorMessage = 'Connection lost unexpectedly';
        } else if (event.reason) {
          errorMessage = event.reason;
        }

        connectionStore.set({ status: 'disconnected', error: errorMessage });
      };

      this.ws.onerror = (event) => {
        console.error('WebSocket error:', event);
        this.errorMessage = 'Connection error';
        connectionStore.set({ status: 'disconnected', error: 'Connection error occurred' });
      };

      this.ws.onmessage = (event) => {
        this.handleMessage(event.data);
      };
    } catch (error) {
      console.error('Failed to create WebSocket:', error);
      this.errorMessage = 'Failed to create WebSocket connection';
      connectionStore.set({ status: 'disconnected', error: this.errorMessage });
    }
  }

  /**
   * Disconnect from the WebSocket server.
   */
  disconnect(): void {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.isConnected = false;
    this.rejectAllPending('WebSocket disconnected by user');
    connectionStore.set({ status: 'disconnected', error: null });
  }

  /**
   * Check if WebSocket is connected and ready.
   */
  isReady(): boolean {
    return this.isConnected && this.ws !== null && this.ws.readyState === WebSocket.OPEN;
  }

  /**
   * Send a request and wait for a response.
   * @param method - Method name
   * @param params - Method parameters
   * @returns Resolves with response data
   */
  request(method: string, params: unknown = {}): Promise<unknown> {
    return new Promise((resolve, reject) => {
      if (!this.isConnected || !this.ws) {
        reject(new Error('WebSocket not connected'));
        return;
      }

      const requestId = crypto.randomUUID();
      
      const request = {
        type: 'request' as const,
        id: requestId,
        method,
        params
      };

      // Validate request structure
      if (request.type !== 'request' || !request.id || !request.method) {
        reject(new Error('Invalid request structure'));
        return;
      }

      // Store pending request
      this.pendingRequests.set(requestId, { resolve, reject });

      // Send request
      try {
        this.ws.send(JSON.stringify(request));
      } catch (error) {
        this.pendingRequests.delete(requestId);
        reject(new Error(`Failed to send request: ${error instanceof Error ? error.message : String(error)}`));
      }
    });
  }

  /**
   * Subscribe to an event type.
   * @param eventName - Event name
   * @param callback - Callback function
   * @returns Unsubscribe function
   */
  on(eventName: string, callback: EventCallback): () => void {
    if (!this.eventListeners.has(eventName)) {
      this.eventListeners.set(eventName, new Set());
    }
    this.eventListeners.get(eventName)!.add(callback);

    // Return unsubscribe function
    return () => {
      const listeners = this.eventListeners.get(eventName);
      if (listeners) {
        listeners.delete(callback);
        if (listeners.size === 0) {
          this.eventListeners.delete(eventName);
        }
      }
    };
  }

  /**
   * Handle incoming WebSocket message.
   * @param data - Raw message data
   */
  private handleMessage(data: string): void {
    let parsed: unknown;
    try {
      parsed = JSON.parse(data);
    } catch (error) {
      console.error('Failed to parse WebSocket message:', error);
      return;
    }

    // Validate message schema
    let message: WebSocketMessage;
    try {
      message = WebSocketMessageSchema.parse(parsed);
    } catch (error) {
      console.error('WebSocket message validation failed:', error);
      console.error('Raw message:', parsed);
      return;
    }

    // Handle response
    if (message.type === 'response') {
      this.handleResponse(message);
    }
    // Handle event
    else if (message.type === 'event') {
      this.handleEvent(message);
    }
  }

  /**
   * Handle response message.
   * @param message - Parsed response message
   */
  private handleResponse(message: Response): void {
    const pending = this.pendingRequests.get(message.id);
    if (!pending) {
      console.warn('Received response for unknown request:', message.id);
      return;
    }

    this.pendingRequests.delete(message.id);

    if (message.success) {
      pending.resolve(message.data);
    } else {
      const errorData = message.data as { error?: string } | null;
      pending.reject(new Error(errorData?.error || 'Request failed'));
    }
  }

  /**
   * Handle event message.
   * @param message - Parsed event message
   */
  private handleEvent(message: Event): void {
    const listeners = this.eventListeners.get(message.event);
    if (!listeners || listeners.size === 0) {
      return;
    }

    // Dispatch event to all listeners
    for (const callback of listeners) {
      try {
        callback(message.data);
      } catch (error) {
        console.error(`Error in event listener for ${message.event}:`, error);
      }
    }
  }

  /**
   * Reject all pending requests.
   * @param reason - Rejection reason
   */
  private rejectAllPending(reason: string): void {
    for (const [, { reject }] of this.pendingRequests) {
      reject(new Error(reason));
    }
    this.pendingRequests.clear();
  }
}

// ============================================================================
// Connection Store
// ============================================================================

/**
 * Connection status store.
 */
export const connectionStore: Writable<ConnectionState> = writable({
  status: 'disconnected',
  error: null
});

// ============================================================================
// Singleton Instance
// ============================================================================

/**
 * WebSocket client singleton.
 */
export const websocket = new WebSocketClient();
