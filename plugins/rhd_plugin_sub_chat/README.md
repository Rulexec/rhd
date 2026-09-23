# rhd_plugin_sub_chat

A plugin that provides the sub-chat delegation tools — `rhd_sub_chat`, `rhd_sub_chat_status`, `rhd_sub_chat_await` — to every chat in the RHD chat system.

## Overview

`rhd_plugin_sub_chat` lets an assistant delegate a self-contained task to a fresh **subchat**: a new chat that runs with its own history and tools and finishes with a final assistant answer. The plugin registers all three tools in every chat (choice-style per-chat registration), so a subchat can itself spawn subchats — recursion works without special-casing.

- `rhd_sub_chat` — spawn a subchat from ordered `system`/`user` seed messages, with optional extra tags; sync mode returns the subchat's final answer, `async=true` returns the new chat id immediately.
- `rhd_sub_chat_status` — `pending` / `completed` check of a direct-child subchat.
- `rhd_sub_chat_await` — hold the tool call until a direct-child subchat finishes, then return its final answer verbatim.

> **Scaffold note.** This phase only registers the tools and owns the tag/template vocabulary. Tool-call handling, subchat spawning, the completion watcher, and crash recovery arrive in the following phases; until then a model calling these tools parks its loop on the unanswered call.

## Trigger Conditions

- On every chat state change (via `ChatMonitor`): if the three tools have not yet been registered for that chat, call `addTools` once per chat with all three definitions. Chats created while the plugin runs register immediately (`chatCreated`); pre-existing chats are picked up on their next state change.
- An in-memory set of initialized chats avoids redundant `addTools` requests. Server-side registration is idempotent (`INSERT OR REPLACE` keyed by chat + plugin + tool name), so plugin restarts and re-delivered events are harmless. On an `addTools` error the chat is deliberately not marked initialized, so the next state change retries.
- Tool calls are **not** handled yet; the `on_tool_call` subscription arrives with the next phases.

## Events Listened For

- Chat state changes only (via `ChatMonitor::subscribe_to_all_chats` + `on_chat_state_change`).
- All custom events are **acknowledged unhandled** (via `on_custom_event`) — the plugin reacts to none, but must never stall senders' all-plugin ack waits (its own subchats run through that pipeline).

## Events Emitted

None.

## Tags Added / Reserved

`src/tags.rs` is the single source of truth for the vocabulary. The plugin adds these tags to a subchat atomically at `createChat` time (spawning activates in a later phase):

- `parent:<chatId>` — direct-parent link.
- `root:<chatId>` — topmost-ancestor lineage (inherited from the spawner's own `root:` tag when present).
- `sub_chat:call:<toolCallId>` — binds the subchat to the spawning tool call (watcher/recovery key).
- `paused` — applied at creation so `ai_completions` does not start the subchat before the spawn is fully wired, then removed once the messages are in. `paused` is already **consumed**: `ai_completions` never triggers on a chat carrying it (exact match, since Phase 1 of this milestone).

These shapes are reserved: user-supplied spawn tags colliding with them (`parent:` / `root:` / `sub_chat:` prefixes, exact `paused`, empty, whitespace) are rejected by `validate_user_tags`.

## Configuration

None — the plugin has no settings and no credentials; CLI arguments only:

```bash
rhd_plugin_sub_chat --server-url ws://localhost:8080/ [--plugin-id rhd_plugin_sub_chat]
```

### Arguments

- `--server-url` (required): WebSocket URL of the chat server
- `--plugin-id` (optional): Plugin identifier (default: `rhd_plugin_sub_chat`)

## Dependencies

- **rhd_chat_client**: For connecting to chat server
- **rhd_chat_api**: Protocol types
- **rhd_plugin_ai_completions**: subchats are ordinary chats; the AI completions plugin actually runs them (and honors the `paused` tag)

## Error Handling

- Startup failures (template loading, connection, plugin registration, pending acks, monitor creation/subscription) abort the plugin with a descriptive `PluginError`.
- Per-chat failures (tool-definition parse errors, `addTools` errors) are logged and the chat is left uninitialized so the next state change retries registration.
- The `on_chat_state_change` callback body is `tokio::spawn`ed — never `await`ed inline in the monitor's dispatch path (see the "Event Callbacks Must Not Block the WebSocket Read Task" pattern in `memory/development.md`).

## Testing

```bash
cargo test -p rhd_plugin_sub_chat
```

Unit tests cover the tag vocabulary and the embedded tool definitions (each parses as a `ToolDefinition` with the expected name); integration tests verify live custom-event acknowledgment and per-chat registration of all three tools in every chat.

## License

Part of the RHD project.
