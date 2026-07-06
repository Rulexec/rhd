# Protocols

## IPC Protocol

- Unix socket at `$HOME/rhd.sock` by default (configurable via `--socket`)
- Message format: 4-byte version + 4-byte length + rkyv payload
- Protocol version: 1
- **Each message** (request or response) includes version prefix
- Request: `IpcRequest::RunScenario { name: String, cwd: String }`, `IpcRequest::Reload`
- Response: `IpcResponse::Success { output: String }`, `IpcResponse::Error { message: String }`, `IpcResponse::Aborted`, `IpcResponse::Paused { error: String, step: String }`, or `IpcResponse::Reloaded { scenarios_reloaded, models_reloaded, mcp_restarted, mcp_stopped, projects_reloaded }`
- Multi-response support: `handle_request` returns `Vec<IpcResponse>` for scenarios that pause then resume

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
- **Server → Client events**: Real-time execution events (scenarioStarted, stepStarted, scenarioFinished, scenarioPaused, scenarioResumed)
  - All event names use **camelCase** (e.g., `scenarioStarted`, not `scenariostarted`)
  - `scenarioFinished` event data uses same `ScenarioMeta` format as `getFinishedScenarios` response items
  - `scenarioPaused` emitted when scenario pauses on AI error (includes executionId, error, stepName, availableModels)
  - `scenarioResumed` emitted when retry starts (includes executionId)
- **Chat streaming events**:
  - `chatStreamChunk`: Contains `chatId` and `content` (incremental text)
  - `chatThinkingChunk`: Contains `chatId` and `content` (incremental thinking/reasoning content)
  - `chatStreamFinished`: Contains `chatId`, `messageId`, and `finishReason`
  - `chatStreamError`: Contains `chatId` and `error` message
  - `chatMessageAdded`: Contains `chatId` and `message` object (user, assistant, or system message persisted)
  - `chatUpdated`: Contains `chatId` and `title` (when chat title changes)
  - `chatToolCallStarted`: Contains `chatId`, `toolCallId`, `toolName`, `arguments`, `mcpName`
  - `chatToolCallCompleted`: Contains `chatId`, `toolCallId`, `result`
  - `chatPaused`: Contains `chatId` (chat paused during tool loop)
  - `chatResumed`: Contains `chatId` (chat resumed from pause)
- Multiple subscribers supported via broadcast channels (separate for execution events and chat events)

## CWD Propagation

- `rhd run` captures its current working directory and sends it to the daemon via IPC
- Commands execute in the client's cwd by default (when `cwd` not explicitly set in scenario YAML)
- If `cwd` is set in scenario YAML, it takes precedence over client's cwd
- The resolved cwd for each `runCommand` step is stored in `StepResult.cwd` and accessible via `%stepName.cwd%` placeholder
- E2E tests run daemon and client in separate directories to verify cwd propagation works correctly

## Reload Command

```bash
rhd reload [--socket PATH]
```

Reloads YAML configs (scenarios, projects, MCP, models) without restarting the daemon. The reload:
- Waits for running scenarios and AI chat streams to finish
- Blocks new scenarios/chats silently until reload completes (no errors)
- Restarts only MCP servers whose configs changed
- Stops MCP servers removed from config (logs PID for manual kill if needed)

Response includes counts: scenarios_reloaded, models_reloaded, mcp_restarted, mcp_stopped, projects_reloaded.
