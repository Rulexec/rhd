# Frontend

Svelte-based web UI in `frontend/` directory for monitoring scenario execution and chat interactions. Written in TypeScript with Zod validation for WebSocket messages.

## Setup

- Requires Node.js v24.13.0 (specified in `.nvmrc`)
- Start with `nvm use && npm run start`
- Type check with `npm run check` (runs svelte-check)
- Build with `npm run build`
- Connects to daemon WebSocket server (default port 9876, configurable via `VITE_WS_PORT` env var)

## Testing

- UI tests use Vitest with happy-dom environment
- Run tests: `cd frontend && ./node_modules/.bin/vitest run`
- Tests spawn `rhd_test frontend` which starts mock AI server, control server, and daemon
- Test files in `src/tests/*.test.ts`
- Test utilities in `src/tests/testUtils.ts`:
  - `waitForWebSocket()` - waits for daemon to be ready
  - `configureMock(content)` - sets mock AI response
  - `getRecordedRequests()` - fetches recorded AI requests
- WebSocket port dynamically set via `setWsPort()` and `connectWebSocket()` from `src/lib/ws.ts`

## Features

### Scenarios Tab
- Shows active scenarios with real-time updates (current step, elapsed time, token counts)
- Abort button to stop active scenario execution
- Shows finished scenarios list (sorted by date, newest first) with status badges
- Status badges: executing (blue), success (green), error (red), aborted (orange)
- Caches finished scenarios; uses `lastId` parameter for incremental fetching

### Chats Tab
- Chat list sidebar with "New Chat" button
- Chat view with message history and streaming responses
- Create, delete, and select chats
- Send messages with Enter (Shift+Enter for newline)
- Edit user messages (truncates subsequent messages and re-streams)
- Abort active streaming responses
- Real-time streaming display with two-phase approach:
  - Phase 1: Loader shown until first chunk arrives
  - Phase 2: Optimistic assistant message with animated dots indicator (cycles "." → ".." → "..." every 200ms)
- Error states with retry button
- Model selector dropdown in message input (shows available models, persists per chat)
- Model indicators in message list (visual dividers showing when model changes between messages)

## Architecture

- WebSocket connection with auto-reconnect
- **Zod validation** for all WebSocket messages (see `src/lib/types/ws.ts`)
- **Event names use camelCase**: `scenarioStarted`, `stepStarted`, `scenarioFinished`, `scenarioPaused`, `scenarioResumed` (not lowercase like `scenariostarted`)
- Svelte stores for state management:
  - Scenario stores: `activeScenarios`, `finishedScenarios`, `pausedScenarios`, `lastKnownId`, `wsConnected`
  - Chat stores: `chats`, `currentChatId`, `messages`, `streamingContent`, `isStreaming`, `streamError`, `streamingMessageId`, `currentChat` (derived)
- CSS modules + utility classes (Tailwind-like approach)
- Components:
  - Layout: `TabNav`, `App`
  - Scenarios: `ScenariosTab`, `ActiveScenario`, `FinishedScenario`, `PausedScenario`
  - Chats: `ChatsTab`, `ChatList`, `ChatView`, `MessageList`, `Message`, `MessageInput`, `StreamingMessage`

## Type System

All types defined with Zod schemas for runtime validation:

- **Domain types** (`src/lib/types/index.ts`): `ActiveScenario`, `FinishedScenario`, `Chat`, `ChatMessage`
- **WebSocket protocol** (`src/lib/types/ws.ts`): `WsMessageSchema`, `WsResponseSchema`, `WsEventSchema` (includes `ScenarioPausedEventSchema`, `ScenarioResumedEventSchema`)

Invalid WebSocket messages are logged and ignored via `safeParse`.

## Chat Stores (`frontend/src/lib/chatStores.ts`)

- `chats`: writable array of chat objects
- `currentChatId`: writable ID of selected chat
- `messages`: writable array of messages for current chat (supports both numeric IDs from backend and string temp IDs for optimistic messages)
- `streamingContent`: writable string accumulating streamed text (legacy, kept for compatibility)
- `isStreaming`: writable boolean indicating active stream
- `streamError`: writable error message (null when no error)
- `streamingMessageId`: writable string|null, tracks the temp ID of the optimistic assistant message during streaming
- `currentChat`: derived store returning current chat object
- `availableModels`: writable array of available model names (fetched from backend)
- `selectedModel`: writable string|null, currently selected model for the active chat

## Chat WebSocket Functions (`frontend/src/lib/chatWs.ts`)

- `loadChats()`: Fetches and populates chat list
- `loadAvailableModels()`: Fetches list of available models from backend
- `createChat(title)`: Creates new chat, selects it
- `selectChat(chatId)`: Loads chat and messages, sets selectedModel from chat's activeModel
- `deleteChat(chatId)`: Removes chat from list
- `sendMessage(content, model)`: Sends message, starts streaming (sets isStreaming=true, does NOT create optimistic message yet)
- `editMessage(messageId, newContent, model)`: Edits message, truncates, re-streams
- `abortChat()`: Aborts active stream
- `handleChatEvent(event, data)`: Processes chat events from WebSocket
  - `chatStreamChunk`: On first chunk, creates optimistic assistant message with temp ID and sets streamingMessageId. On subsequent chunks, appends content to the optimistic message.
  - `chatStreamFinished`: Sets isStreaming=false, waits for chatMessageAdded to replace optimistic message
  - `chatMessageAdded`: Replaces optimistic message (matched by streamingMessageId) with real message from backend
  - `chatStreamError`: Clears streamingMessageId, sets isStreaming=false

## Chat Components

- **`ChatsTab.svelte`**: Main chat tab layout with sidebar and view area
- **`ChatList.svelte`**: Sidebar with chat list, new chat button, delete buttons
- **`ChatView.svelte`**: Main chat area with header, message list, and input
- **`MessageList.svelte`**: Scrollable message list with auto-scroll on new content, shows model indicators when model changes between messages. Shows StreamingMessage loader only when isStreaming=true AND streamingMessageId is null (before first chunk arrives).
- **`Message.svelte`**: Individual message display with edit mode for user messages. When message.id matches streamingMessageId, displays animated dots indicator (CSS animation cycling through ".", "..", "...")
- **`MessageInput.svelte`**: Textarea with send/abort buttons, error display with retry, model selector dropdown
- **`StreamingMessage.svelte`**: Loading dots animation shown before first streaming chunk arrives

### Model Selection UI

The `MessageInput` component includes a model selector dropdown that:
- Fetches available models on mount via `loadAvailableModels()`
- Binds to the `selectedModel` store
- Shows only real models (aliases are filtered out on backend)
- Persists the selected model per chat (stored in `chats.active_model`)
- Auto-selects the first available model when no model is currently selected

The `MessageList` component displays model indicators:
- Shows a visual divider when the model changes between messages
- Indicators are purely visual and not sent to the AI
- Helps users track which model was used for each part of the conversation
