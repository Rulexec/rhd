import { writable } from 'svelte/store';
import {
  WebSocketMessageSchema,
  ResponseSchema,
  EventSchema
} from './schemas.js';

const WS_URL = 'ws://localhost:8080';

/**
 * WebSocket client for chat server communication.
 * Handles connection lifecycle, request/response correlation, and event dispatch.
 */
class WebSocketClient {
  constructor() {
    /** @type {WebSocket | null} */
    this.ws = null;
    
    /** @type {Map<string, { resolve: Function, reject: Function }>} */
    this.pendingRequests = new Map();
    
    /** @type {Map<string, Set<Function>>} */
    this.eventListeners = new Map();
    
    /** @type {boolean} */
    this.isConnected = false;
    
    /** @type {string | null} */
    this.errorMessage = null;
  }

  /**
   * Connect to the WebSocket server.
   */
  connect() {
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

      this.ws.onclose = () => {
        console.log('WebSocket disconnected');
        this.isConnected = false;
        this.ws = null;
        this.rejectAllPending('WebSocket connection closed');
        connectionStore.set({ status: 'disconnected', error: 'Connection closed' });
      };

      this.ws.onerror = (event) => {
        console.error('WebSocket error:', event);
        this.errorMessage = 'Connection error';
        connectionStore.set({ status: 'disconnected', error: 'Connection error' });
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
  disconnect() {
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.isConnected = false;
    this.rejectAllPending('WebSocket disconnected by user');
    connectionStore.set({ status: 'disconnected', error: null });
  }

  /**
   * Send a request and wait for a response.
   * @param {string} method - Method name
   * @param {any} params - Method parameters
   * @returns {Promise<any>} - Resolves with response data
   */
  request(method, params = {}) {
    return new Promise((resolve, reject) => {
      if (!this.isConnected || !this.ws) {
        reject(new Error('WebSocket not connected'));
        return;
      }

      const requestId = crypto.randomUUID();
      
      const request = {
        type: 'request',
        id: requestId,
        method,
        params
      };

      // Validate request schema
      try {
        // We don't need to validate outgoing requests, but we ensure the structure is correct
        if (request.type !== 'request' || !request.id || !request.method) {
          throw new Error('Invalid request structure');
        }
      } catch (error) {
        reject(new Error(`Invalid request: ${error.message}`));
        return;
      }

      // Store pending request
      this.pendingRequests.set(requestId, { resolve, reject });

      // Send request
      try {
        this.ws.send(JSON.stringify(request));
      } catch (error) {
        this.pendingRequests.delete(requestId);
        reject(new Error(`Failed to send request: ${error.message}`));
      }
    });
  }

  /**
   * Subscribe to an event type.
   * @param {string} eventName - Event name
   * @param {Function} callback - Callback function
   * @returns {Function} - Unsubscribe function
   */
  on(eventName, callback) {
    if (!this.eventListeners.has(eventName)) {
      this.eventListeners.set(eventName, new Set());
    }
    this.eventListeners.get(eventName).add(callback);

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
   * @param {string} data - Raw message data
   */
  handleMessage(data) {
    let parsed;
    try {
      parsed = JSON.parse(data);
    } catch (error) {
      console.error('Failed to parse WebSocket message:', error);
      return;
    }

    // Validate message schema
    let message;
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
   * @param {any} message - Parsed response message
   */
  handleResponse(message) {
    const pending = this.pendingRequests.get(message.id);
    if (!pending) {
      console.warn('Received response for unknown request:', message.id);
      return;
    }

    this.pendingRequests.delete(message.id);

    if (message.success) {
      pending.resolve(message.data);
    } else {
      pending.reject(new Error(message.data?.error || 'Request failed'));
    }
  }

  /**
   * Handle event message.
   * @param {any} message - Parsed event message
   */
  handleEvent(message) {
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
   * @param {string} reason - Rejection reason
   */
  rejectAllPending(reason) {
    for (const [id, { reject }] of this.pendingRequests) {
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
 * @type {import('svelte/store').Writable<{ status: 'connected' | 'disconnected' | 'connecting', error: string | null }>}
 */
export const connectionStore = writable({
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
