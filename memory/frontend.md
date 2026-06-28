# Frontend

Svelte-based web UI in `frontend/` directory for monitoring scenario execution and chat interactions. Written in TypeScript with Zod validation for WebSocket messages.

## Setup

- Requires Node.js v24.13.0 (specified in `.nvmrc`)
- Start with `nvm use && npm run start`
- Type check with `npm run check` (runs svelte-check)
- Build with `npm run build`
- Connects to daemon WebSocket server (default port 9876, configurable via `VITE_WS_PORT` env var)

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
- Real-time streaming display with loading indicator
- Error states with retry button
- Model selector dropdown in message input (shows available models, persists per chat)
- Model indicators in message list (visual dividers showing when model changes between messages)

## Architecture

- WebSocket connection with auto-reconnect
- **Zod validation** for all WebSocket messages (see `src/lib/types/ws.ts`)
- Svelte stores for state management:
  - Scenario stores: `activeScenarios`, `finishedScenarios`, `lastKnownId`, `wsConnected`
  - Chat stores: `chats`, `currentChatId`, `messages`, `streamingContent`, `isStreaming`, `streamError`, `currentChat` (derived)
- CSS modules + utility classes (Tailwind-like approach)
- Components:
  - Layout: `TabNav`, `App`
  - Scenarios: `ScenariosTab`, `ActiveScenario`, `FinishedScenario`
  - Chats: `ChatsTab`, `ChatList`, `ChatView`, `MessageList`, `Message`, `MessageInput`, `StreamingMessage`

## Type System

All types defined with Zod schemas for runtime validation:

- **Domain types** (`src/lib/types/index.ts`): `ActiveScenario`, `FinishedScenario`, `Chat`, `ChatMessage`
- **WebSocket protocol** (`src/lib/types/ws.ts`): `WsMessageSchema`, `WsResponseSchema`, `WsEventSchema`

Invalid WebSocket messages are logged and ignored via `safeParse`.

## Chat Stores (`frontend/src/lib/chatStores.ts`)

- `chats`: writable array of chat objects
- `currentChatId`: writable ID of selected chat
- `messages`: writable array of messages for current chat
- `streamingContent`: writable string accumulating streamed text
- `isStreaming`: writable boolean indicating active stream
- `streamError`: writable error message (null when no error)
- `currentChat`: derived store returning current chat object
- `availableModels`: writable array of available model names (fetched from backend)
- `selectedModel`: writable string|null, currently selected model for the active chat

## Chat WebSocket Functions (`frontend/src/lib/chatWs.ts`)

- `loadChats()`: Fetches and populates chat list
- `loadAvailableModels()`: Fetches list of available models from backend
- `createChat(title)`: Creates new chat, selects it
- `selectChat(chatId)`: Loads chat and messages, sets selectedModel from chat's activeModel
- `deleteChat(chatId)`: Removes chat from list
- `sendMessage(content, model)`: Sends message, starts streaming
- `editMessage(messageId, newContent, model)`: Edits message, truncates, re-streams
- `abortChat()`: Aborts active stream
- `handleChatEvent(event, data)`: Processes chat events from WebSocket

## Chat Components

- **`ChatsTab.svelte`**: Main chat tab layout with sidebar and view area
- **`ChatList.svelte`**: Sidebar with chat list, new chat button, delete buttons
- **`ChatView.svelte`**: Main chat area with header, message list, and input
- **`MessageList.svelte`**: Scrollable message list with auto-scroll on new content, shows model indicators when model changes between messages
- **`Message.svelte`**: Individual message display with edit mode for user messages
- **`MessageInput.svelte`**: Textarea with send/abort buttons, error display with retry, model selector dropdown
- **`StreamingMessage.svelte`**: Streaming response display with loading dots animation

### Model Selection UI

The `MessageInput` component includes a model selector dropdown that:
- Fetches available models on mount via `loadAvailableModels()`
- Binds to the `selectedModel` store
- Shows only real models (aliases are filtered out on backend)
- Persists the selected model per chat (stored in `chats.active_model`)

The `MessageList` component displays model indicators:
- Shows a visual divider when the model changes between messages
- Indicators are purely visual and not sent to the AI
- Helps users track which model was used for each part of the conversation
