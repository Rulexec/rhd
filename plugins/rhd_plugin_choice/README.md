# rhd_plugin_choice

A plugin that provides the `rhd_choice` decision tool to every chat in the RHD chat system.

## Overview

`rhd_plugin_choice` registers the `rhd_choice` tool in every chat — newly created ones and pre-existing ones (startup reconciliation) — so the assistant can ask the user to choose between concrete options before continuing. The frontend renders the question as clickable option buttons plus a free-text input and answers the tool call on the user's behalf.

**This plugin never answers tool calls.** It only registers the tool; responding to `rhd_choice` calls is the frontend's responsibility (see Tool Contract below).

## Trigger Conditions

- On every chat state change (new chat, or pre-existing chats during startup reconciliation): if the `rhd_choice` tool has not yet been registered for that chat, call `addTools` once per chat.
- An in-memory set of initialized chats avoids redundant `addTools` requests. Server-side registration is idempotent (`INSERT OR REPLACE` keyed by chat + plugin + tool name), so plugin restarts and re-delivered events are harmless. On an `addTools` error the chat is deliberately not marked initialized, so the next state change retries.

## Events Listened For

- Chat state changes only (via `ChatMonitor::subscribe_to_all_chats` + `on_chat_state_change`).
- All custom events are **acknowledged unhandled** (via `on_custom_event`) — the plugin reacts to none, but must never stall senders' all-plugin ack waits.

## Events Emitted

None.

## Tags Added

None.

## Tool Contract

| Item | Value |
|---|---|
| Tool name | `rhd_choice` |
| Params | `{ question: string, options: string[] }` (both required) |
| Plugin ID (default) | `choice` |
| Answer | `addMessage` with `role: "tool"`, `toolCallId`, `content` = the exact text of the chosen option or a user-typed message — posted by the **frontend**, never by this plugin |

The tool definition lives in `templates/mcp_internal/rhd_choice/tool_definition.json` and is fully self-describing, so no contract system message is injected (unlike `rhd_plugin_todo_list`) — the definition reaches the model in every request's `tools` array.

## Behavior Notes

- **Unanswered `rhd_choice` calls intentionally pause the `ai_completions` tool loop.** The AI plugin requires all tool calls resolved before continuing, so the conversation waits until a human answers in the UI — that is the point of the feature.
- The plugin handles no custom events, but it **must still acknowledge them**: it subscribes via `on_custom_event` and acks everything unhandled, and drains `getPendingAcks` at startup. A registered plugin that never acks stalls senders' all-plugin coordination waits (e.g. `ai_completions:preRequest` / `ai_completions:preDrainQueue` park the chat on a 30 s timeout) — the startup-only drain was insufficient because events delivered *while running* were left un-acked (regression seen with `preDrainQueue`).
- The `on_chat_state_change` callback body is `tokio::spawn`ed — never `await`ed inline in the monitor's dispatch path (see the "Event Callbacks Must Not Block the WebSocket Read Task" pattern in `memory/development.md`).

## Configuration

```bash
rhd_plugin_choice --server-url ws://localhost:8080/ [--plugin-id choice]
```

### Arguments

- `--server-url` (required): WebSocket URL of the chat server
- `--plugin-id` (optional): Plugin identifier (default: `choice`)

## Dependencies

- **rhd_chat_client**: For connecting to chat server
- **rhd_chat_api**: Protocol types

## Error Handling

- Startup failures (template loading, connection, plugin registration, pending acks, monitor creation/subscription) abort the plugin with a descriptive `PluginError`.
- Per-chat failures (tool-definition parse errors, `addTools` errors) are logged and the chat is left uninitialized so the next state change retries registration.

## Testing

```bash
cargo test -p rhd_plugin_choice
```

Integration tests verify the embedded template loads, the tool definition is valid JSON, and it matches the contract (name, parameters, required fields).

## License

Part of the RHD project.
