# Phase 1: Project Setup & WebSocket Foundation

## Overview

Establish the Svelte project structure, configure build tools, and implement the WebSocket connection layer with reactive state management and Zod type validation. This phase creates the foundation that all other phases build upon.

**Scope:**
- Svelte project initialization with Vite
- WebSocket client with request/response correlation
- Zod schemas for all data types
- Connection status store
- Global CSS variables and base styles
- Basic app shell with tab placeholder

**Out of Scope:**
- Chat list UI (Phase 2)
- Chat view UI (Phase 3)
- Plugins UI (Phase 5)
- Error handling UI (Phase 6)

## Files to Create

### 1. `frontend/package.json`

**Purpose:** Define project dependencies and scripts.

```json
{
  "name": "rhd-chat-frontend",
  "private": true,
  "version": "0.0.1",
  "type": "module",
  "scripts": {
    "dev": "vite dev",
    "build": "vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "marked": "^15.0.0",
    "zod": "^3.23.0"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "^5.0.0",
    "svelte": "^5.0.0",
    "vite": "^6.0.0"
  }
}
```

### 2. `frontend/vite.config.js`

**Purpose:** Configure Vite build tool with Svelte plugin.

```javascript
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  server: {
    port: 5173,
    strictPort: true
  },
  css: {
    modules: {
      localsConvention: 'camelCase'
    }
  }
});
```

### 3. `frontend/svelte.config.js`

**Purpose:** Configure Svelte compiler options.

```javascript
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

export default {
  preprocess: vitePreprocess(),
  compilerOptions: {
    runes: true
  }
};
```

### 4. `frontend/index.html`

**Purpose:** Entry HTML file that loads the Svelte app.

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>RHD Chat</title>
  <link rel="stylesheet" href="/src/global.css" />
</head>
<body>
  <div id="app"></div>
  <script type="module" src="/src/main.js"></script>
</body>
</html>
```

### 5. `frontend/src/main.js`

**Purpose:** Application entry point, initializes the app.

```javascript
import { mount } from 'svelte';
import App from './App.svelte';

const app = mount(App, {
  target: document.getElementById('app')
});

export default app;
```

### 6. `frontend/src/App.svelte`

**Purpose:** Root component with basic structure and tab placeholder.

```svelte
<script>
  import { connectionStore } from './lib/stores/connection.js';
  import { websocket } from './lib/api/websocket.js';

  // Connect to WebSocket on mount
  websocket.connect();
</script>

<div class="app">
  <header class="app-header">
    <h1>RHD Chat</h1>
  </header>
  
  <main class="app-main">
    <p>Tab content will be rendered here (Phase 2+)</p>
  </main>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--color-bg);
    color: var(--color-text);
  }

  .app-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .app-header h1 {
    margin: 0;
    font-size: var(--font-size-lg);
  }

  .app-main {
    flex: 1;
    overflow: hidden;
  }
</style>
```

### 7. `frontend/src/global.css`

**Purpose:** Global CSS variables for colors, sizes, and base styles.

```css
:root {
  /* Colors */
  --color-primary: #3b82f6;
  --color-primary-hover: #2563eb;
  --color-secondary: #6b7280;
  --color-secondary-hover: #4b5563;
  --color-bg: #ffffff;
  --color-bg-secondary: #f9fafb;
  --color-bg-tertiary: #f3f4f6;
  --color-text: #111827;
  --color-text-secondary: #6b7280;
  --color-text-muted: #9ca3af;
  --color-border: #e5e7eb;
  --color-error: #ef4444;
  --color-error-bg: #fef2f2;
  --color-success: #10b981;
  --color-success-bg: #ecfdf5;
  --color-warning: #f59e0b;
  --color-warning-bg: #fffbeb;

  /* Spacing */
  --spacing-xs: 4px;
  --spacing-sm: 8px;
  --spacing-md: 16px;
  --spacing-lg: 24px;
  --spacing-xl: 32px;

  /* Border Radius */
  --radius-sm: 4px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-full: 9999px;

  /* Font Sizes */
  --font-size-xs: 12px;
  --font-size-sm: 14px;
  --font-size-md: 16px;
  --font-size-lg: 18px;
  --font-size-xl: 24px;

  /* Shadows */
  --shadow-sm: 0 1px 2px 0 rgba(0, 0, 0, 0.05);
  --shadow-md: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
  --shadow-lg: 0 10px 15px -3px rgba(0, 0, 0, 0.1);

  /* Transitions */
  --transition-fast: 150ms ease;
  --transition-normal: 250ms ease;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
  font-size: var(--font-size-md);
  line-height: 1.5;
  color: var(--color-text);
  background: var(--color-bg);
}

button {
  cursor: pointer;
  font-family: inherit;
  font-size: inherit;
}

input, textarea {
  font-family: inherit;
  font-size: inherit;
}
```

### 8. `frontend/src/lib/api/schemas.js`

**Purpose:** Zod schemas for all WebSocket message types and data structures.

```javascript
import { z } from 'zod';

// ============================================================================
// Data Structure Schemas
// ============================================================================

/**
 * Chat summary schema.
 * Used in chat lists and chat events.
 */
export const ChatSchema = z.object({
  id: z.number(),
  title: z.string(),
  createdAt: z.string(), // ISO 8601 date string
  updatedAt: z.string(), // ISO 8601 date string
  tags: z.array(z.string()),
  version: z.number()
});

/**
 * Message schema.
 * Used for both regular messages and queue messages.
 */
export const MessageSchema = z.object({
  id: z.number(),
  chatId: z.number(),
  role: z.string(),
  content: z.string(),
  createdAt: z.string(), // ISO 8601 date string
  reasoningContent: z.string().optional(),
  tags: z.array(z.string()).default([])
});

/**
 * Plugin summary schema.
 * Used in plugin list and plugin events.
 */
export const PluginSummarySchema = z.object({
  pluginId: z.string(),
  isActive: z.boolean()
});

// ============================================================================
// WebSocket Protocol Schemas
// ============================================================================

/**
 * Request schema (client → server).
 */
export const RequestSchema = z.object({
  type: z.literal('request'),
  id: z.string(),
  method: z.string(),
  params: z.any().default({})
});

/**
 * Response schema (server → client).
 */
export const ResponseSchema = z.object({
  type: z.literal('response'),
  id: z.string(),
  success: z.boolean(),
  data: z.any()
});

/**
 * Event schema (server → client).
 */
export const EventSchema = z.object({
  type: z.literal('event'),
  event: z.string(),
  data: z.any()
});

/**
 * Generic WebSocket message schema.
 * Used to parse incoming messages and determine their type.
 */
export const WebSocketMessageSchema = z.discriminatedUnion('type', [
  ResponseSchema,
  EventSchema
]);

// ============================================================================
// Event Data Schemas
// ============================================================================

/**
 * Chat created event data.
 */
export const ChatCreatedDataSchema = z.object({
  chat: ChatSchema,
  chatVersion: z.number()
});

/**
 * Chat updated event data.
 */
export const ChatUpdatedDataSchema = z.object({
  chat: ChatSchema,
  chatVersion: z.number()
});

/**
 * Chat deleted event data.
 */
export const ChatDeletedDataSchema = z.object({
  chatId: z.number(),
  chatVersion: z.number()
});

/**
 * Message added event data.
 */
export const MessageAddedDataSchema = z.object({
  chatId: z.number(),
  message: MessageSchema,
  chatVersion: z.number()
});

/**
 * Message updated event data.
 */
export const MessageUpdatedDataSchema = z.object({
  chatId: z.number(),
  message: MessageSchema,
  chatVersion: z.number()
});

/**
 * Message deleted event data.
 */
export const MessageDeletedDataSchema = z.object({
  chatId: z.number(),
  messageId: z.number(),
  chatVersion: z.number()
});

/**
 * Queue message added event data.
 */
export const QueueMessageAddedDataSchema = z.object({
  chatId: z.number(),
  message: MessageSchema,
  chatVersion: z.number()
});

/**
 * Queue message updated event data.
 */
export const QueueMessageUpdatedDataSchema = z.object({
  chatId: z.number(),
  message: MessageSchema,
  chatVersion: z.number()
});

/**
 * Queue message deleted event data.
 */
export const QueueMessageDeletedDataSchema = z.object({
  chatId: z.number(),
  messageId: z.number(),
  chatVersion: z.number()
});

/**
 * Plugin registered event data.
 */
export const PluginRegisteredDataSchema = z.object({
  plugin: PluginSummarySchema
});

/**
 * Plugin updated event data.
 */
export const PluginUpdatedDataSchema = z.object({
  plugin: PluginSummarySchema
});

/**
 * Plugin removed event data.
 */
export const PluginRemovedDataSchema = z.object({
  pluginId: z.string()
});

// ============================================================================
// Method Result Schemas
// ============================================================================

/**
 * List chats result.
 */
export const ListChatsResultSchema = z.object({
  chats: z.array(ChatSchema)
});

/**
 * Create chat result.
 */
export const CreateChatResultSchema = z.object({
  chat: ChatSchema
});

/**
 * Get chat result.
 */
export const GetChatResultSchema = z.object({
  chat: ChatSchema,
  messages: z.array(MessageSchema)
});

/**
 * Get queue messages result.
 */
export const GetQueueMessagesResultSchema = z.object({
  messages: z.array(MessageSchema)
});

/**
 * Get plugins result.
 */
export const GetPluginsResultSchema = z.object({
  plugins: z.array(PluginSummarySchema)
});

// ============================================================================
// Type Exports (for JSDoc type inference)
// ============================================================================

/** @typedef {z.infer<typeof ChatSchema>} Chat */
/** @typedef {z.infer<typeof MessageSchema>} Message */
/** @typedef {z.infer<typeof PluginSummarySchema>} PluginSummary */
/** @typedef {z.infer<typeof RequestSchema>} Request */
/** @typedef {z.infer<typeof ResponseSchema>} Response */
/** @typedef {z.infer<typeof EventSchema>} Event */
/** @typedef {z.infer<typeof ListChatsResultSchema>} ListChatsResult */
/** @typedef {z.infer<typeof CreateChatResultSchema>} CreateChatResult */
/** @typedef {z.infer<typeof GetChatResultSchema>} GetChatResult */
/** @typedef {z.infer<typeof GetQueueMessagesResultSchema>} GetQueueMessagesResult */
/** @typedef {z.infer<typeof GetPluginsResultSchema>} GetPluginsResult */
```

### 9. `frontend/src/lib/api/websocket.js`

**Purpose:** WebSocket connection management with event handling and Zod validation.

```javascript
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
```

### 10. `frontend/src/lib/stores/connection.js`

**Purpose:** Svelte store for connection status (re-export from websocket.js for convenience).

```javascript
/**
 * Re-export connection store from websocket module.
 * This provides a convenient import path for components.
 */
export { connectionStore } from '../api/websocket.js';
```

## Tests

### Manual Testing

1. **Project Setup:**
   - Run `cd frontend && npm install`
   - Run `npm run dev`
   - Verify app loads at http://localhost:5173
   - Verify no console errors

2. **WebSocket Connection:**
   - Start chat server: `cargo run --bin rhd_chat_server`
   - Open browser console
   - Verify "WebSocket connected" message appears
   - Verify connection store shows status: 'connected'

3. **Zod Validation:**
   - Open browser console
   - Import schemas: `import { ChatSchema } from './src/lib/api/schemas.js'`
   - Test valid data: `ChatSchema.parse({ id: 1, title: 'Test', createdAt: '2026-08-23T20:00:00Z', updatedAt: '2026-08-23T20:00:00Z', tags: [], version: 1 })`
   - Test invalid data: `ChatSchema.parse({ id: 'not-a-number' })` - should throw validation error

4. **CSS Variables:**
   - Inspect elements in browser dev tools
   - Verify CSS variables are applied (colors, spacing, etc.)

## Implementation Notes

1. **WebSocket URL**: Hardcoded to `ws://localhost:8080` for now. Can be made configurable via environment variable in future.

2. **Zod Validation**: All incoming WebSocket messages are validated against schemas. Validation errors are logged to console with details about the mismatch, making it easy to debug frontend-backend type mismatches.

3. **Request/Response Correlation**: Each request gets a unique UUID. The WebSocket client maintains a map of pending requests and resolves/rejects them when responses arrive.

4. **Event System**: Components can subscribe to specific event types using `websocket.on(eventName, callback)`. The method returns an unsubscribe function for cleanup.

5. **Connection Store**: Uses Svelte's writable store for reactive connection status. Components can subscribe to this store to show connection indicators.

6. **Error Handling**: WebSocket errors update the connection store. Pending requests are rejected with clear error messages when the connection is lost.

## Dependencies

- **None** - This is the foundation phase
- All subsequent phases depend on this phase

## Success Criteria

- [ ] Svelte app builds and runs without errors
- [ ] WebSocket connects to `ws://localhost:8080` successfully
- [ ] Connection status is tracked and accessible via store
- [ ] Global CSS variables are defined and applied
- [ ] Basic app structure renders with header
- [ ] Zod schemas defined for all core data types (Chat, Message, PluginSummary)
- [ ] Zod schemas defined for all WebSocket message types (Request, Response, Event)
- [ ] Zod schemas defined for all event data types
- [ ] WebSocket messages validated against Zod schemas
- [ ] Validation errors logged with clear details about type mismatches
- [ ] Request/response correlation works correctly
- [ ] Event subscription system works correctly
