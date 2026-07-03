# Protocols

## IPC Protocol

- Unix socket at `$HOME/rhd.sock` by default (configurable via `--socket`)
- Message format: 4-byte version + 4-byte length + rkyv payload
- Protocol version: 1
- Request: `IpcRequest::RunScenario { name: String, cwd: String }`
- Response: `IpcResponse::Success { output: String }`, `IpcResponse::Error { message: String }`, or `IpcResponse::Aborted`

## WebSocket Protocol

- Optional TCP listener on `127.0.0.1:{ws_port}` (configurable via `--ws-port` or `wsPort` in config)
- JSON over WebSocket for web-friendly integration
- **Client → Server requests**:
  - `runScenario`: Execute a scenario
  - `subscribe`: Subscribe to execution events (returns list of currently active executions)
  - `getFinishedScenarios`: Get list of finished scenarios from meta.json. Accepts optional `lastId` parameter to fetch only scenarios with id > lastId (for incremental updates)
  - `abortScenario`: Abort an active scenario execution by execution ID
  - `createChat`: Create a new chat with title
  - `listChats`: Get list of all chats (sorted by updated_at DESC)
  - `getChat`: Get chat info and messages by chat_id
  - `deleteChat`: Delete a chat and all its messages
  - `sendMessage`: Send a message to a chat and stream AI response
  - `editMessage`: Edit a user message, truncate subsequent messages, and re-stream AI response
  - `abortChat`: Abort an active streaming response in a chat
  - `getAvailableModels`: Get list of available models (real models only, excludes aliases)
- **Server → Client responses**: Request responses with success/error status
- **Server → Client events**: Real-time execution events (scenarioStarted, stepStarted, scenarioFinished)
  - `scenarioFinished` event data uses same `ScenarioMeta` format as `getFinishedScenarios` response items
- **Chat streaming events**:
  - `chatStreamChunk`: Contains `chatId` and `content` (incremental text)
  - `chatStreamFinished`: Contains `chatId`, `messageId`, and `finishReason`
  - `chatStreamError`: Contains `chatId` and `error` message
  - `chatMessageAdded`: Contains `chatId` and `message` object (user or assistant message persisted)
  - `chatUpdated`: Contains `chatId` and `title` (when chat title changes)
- Multiple subscribers supported via broadcast channels (separate for execution events and chat events)

## CWD Propagation

- `rhd run` captures its current working directory and sends it to the daemon via IPC
- Commands execute in the client's cwd by default (when `cwd` not explicitly set in scenario YAML)
- If `cwd` is set in scenario YAML, it takes precedence over client's cwd
- The resolved cwd for each `runCommand` step is stored in `StepResult.cwd` and accessible via `%stepName.cwd%` placeholder
- E2E tests run daemon and client in separate directories to verify cwd propagation works correctly
