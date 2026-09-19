# Protocols

JSON over WebSocket between clients (frontend, CLI, plugins via `rhd_chat_client`) and `rhd_chat_server`. The default endpoint is `ws://127.0.0.1:8080` (server flags `--host`/`--port`).

All message type/event names use **camelCase** (e.g., `pluginStateChanged`, not `pluginstatechanged`).

## Message Envelopes

Defined in `packages/rhd_chat_api/src/protocol.rs`:

- **Request** (client → server): `{ "type": "request", "id": "<uuid>", "method": "<name>", "params": {…} }`
- **Response** (server → client): `{ "type": "response", "id": "<uuid>", "success": true, "data": {…} }` — matched to request by `id`; errors carry an error status/message instead
- **Event** (server → client): `{ "type": "event", "event": "<name>", "data": {…} }` — pushed to subscribed connections

## Methods

Source of truth: `packages/rhd_chat_api/src/methods/`.

- **Chats**: `createChat`, `listChats`, `getChat`, `updateChat`, `deleteChat`
- **Messages**: `addMessage`, `getMessages`, `updateMessage`, `deleteMessage`
- **Queue**: `addQueueMessage` (optional `beforeMessageId` inserts the new message directly before the referenced one; the queue is ordered by an internal `position`, `position ASC, id ASC`), `getQueueMessages`, `updateQueueMessage`, `deleteQueueMessage`
- **Subscriptions**: `subscribeChat`/`unsubscribeChat`, `subscribeChatsList`/`unsubscribeChatsList`, `subscribePluginsList`/`unsubscribePluginsList`
- **Streams** (plugin-driven AI streaming): `streamPush` (accumulate deltas), `streamSubscribe` (get current state + subscribe to `streamChunk`), `streamFinish`
- **Plugins**: `registerPlugin`, `getPlugins`, `removePlugin`
- **Tools**: `addTools`, `removeTools`, `getTools`, `updateToolCallTags`
- **Plugin states**: `updatePluginState`, `removePluginState`, `getPluginStates`, `subscribePluginStates`, `unsubscribePluginStates` — semantics in [architecture.md](architecture.md) (register-before-snapshot, version gating)
- **Custom events**: `sendCustomEvent`, `ackCustomEvent`, `getPendingAcks`

## Events

Source of truth: `packages/rhd_chat_api/src/events/`.

- **Chats**: `chatCreated`, `chatUpdated`, `chatDeleted`
- **Messages**: `messageAdded`, `messageUpdated`, `messageDeleted`, `assistantMessageWithToolCalls`
- **Queue**: `queueMessageAdded`, `queueMessageUpdated`, `queueMessageDeleted`
- **Streams**: `streamChunk` (delta type: reasoningDelta/contentDelta/toolCallDelta), `streamFinished`
- **Plugins**: `pluginRegistered`, `pluginUpdated`, `pluginRemoved`
- **Plugin states**: `pluginStateChanged`, `pluginStateRemoved`
- **Tools**: `toolsUpdated`
- **Custom events**: `customEvent`, `customEventAcknowledged`

## Related

- StreamManager internals (server-side stream accumulation): [architecture.md](architecture.md)
- Client-side subscription/callback API (`rhd_chat_client`): [architecture.md](architecture.md)
