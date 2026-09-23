# rhd_plugin_sub_chat

A plugin that provides the sub-chat delegation tools — `rhd_sub_chat`, `rhd_sub_chat_status`, `rhd_sub_chat_await` — to every chat in the RHD chat system.

## Overview

`rhd_plugin_sub_chat` lets an assistant delegate a self-contained task to a fresh **subchat**: a new chat that runs with its own history and tools and finishes with a final assistant answer. The plugin registers all three tools in every chat (choice-style per-chat registration), so a subchat can itself spawn subchats — recursion works without special-casing.

- `rhd_sub_chat` — spawn a subchat from ordered `system`/`user` seed messages, with optional extra tags; sync mode returns the subchat's final answer, `async=true` returns the new chat id immediately.
- `rhd_sub_chat_status` — `pending` / `completed` check of a direct-child subchat.
- `rhd_sub_chat_await` — hold the tool call until a direct-child subchat finishes, then return its final answer verbatim.

> **Status note.** Tool registration and the `rhd_sub_chat` spawn engine (async mode) are live. Sync-mode spawns fully activate the subchat but deliberately leave the tool call **unanswered** — the completion watcher that resolves sync calls lands in Phase 4, and the `rhd_sub_chat_status` / `rhd_sub_chat_await` handlers in Phase 5 (their events already fall through the subscription). Crash recovery arrives in Phase 6; the converge primitive is already restart-safe by design.

## Trigger Conditions

- On every chat state change (via `ChatMonitor`): if the three tools have not yet been registered for that chat, call `addTools` once per chat with all three definitions. Chats created while the plugin runs register immediately (`chatCreated`); pre-existing chats are picked up on their next state change.
- An in-memory set of initialized chats avoids redundant `addTools` requests. Server-side registration is idempotent (`INSERT OR REPLACE` keyed by chat + plugin + tool name), so plugin restarts and re-delivered events are harmless. On an `addTools` error the chat is deliberately not marked initialized, so the next state change retries registration.
- On `assistantMessageWithToolCalls` for any chat (`on_tool_call(0, …)`, subscribed once with all three tool names): each `rhd_sub_chat` call runs the spawn flow:
  1. **Guard:** a `tool` message answering this `tool_call_id` already exists → skip (duplicate-answer protection; `reply::has_tool_result`).
  2. **Validate:** roles restricted to `system`/`user`, non-empty `messages`, string contents, user tags free of reserved shapes — failures are answered as a single-line `rhd_sub_chat error: …` with **no chat created**.
  3. **Converge** (`spawn::converge_spawn`, the two-phase activation protocol and future recovery kernel): ensure the caller's `root:` tag (self-tag the caller when absent) → find the subchat by its `sub_chat:call:<toolCallId>` link tag or create it **in one atomic `createChat`** with the full tag set (`parent:`, `root:`, link, user tags, `paused`) → while still `paused`, reconcile the message queue against the plan prefix (a tampered queue → refuse, error the call, leave the subchat paused) and enqueue missing starting messages strictly in order → **remove `paused` last** (the activation point — `ai_completions` never sees a half-populated subchat).
  4. **Answer:** `async: true` → the fixed notice `Chat started in background with id <chatId>, use tools rhd_sub_chat_status or rhd_sub_chat_await on it`. Sync → the call stays unanswered (parking the caller's tool loop) through an explicit seam until Phase 4's watcher. Mid-spawn server failures are answered `rhd_sub_chat error: failed to prepare subchat <id|new>: <detail>`; with a known id the "(subchat left paused for recovery)" suffix is appended and the paused subchat is left intact.

## Events Listened For

- Chat state changes (via `ChatMonitor::subscribe_to_all_chats` + `on_chat_state_change`).
- `assistantMessageWithToolCalls` for the three sub-chat tool names, all chats (via `on_tool_call(0, …)`; the client dispatcher runs the callback in a spawned task, so inline client requests inside the handler are safe).
- All custom events are **acknowledged unhandled** (via `on_custom_event`) — the plugin reacts to none, but must never stall senders' all-plugin ack waits (its own subchats run through that pipeline).

## Events Emitted

No custom events. The plugin writes ordinary chat state: created subchats (`chatCreated`), queued starting messages (`queueMessageAdded`), tag updates (`chatUpdated`, incl. the `paused` removal that activates the subchat), and `tool`-role answers in caller chats (`messageAdded`).

## Tags Added / Reserved

`src/tags.rs` is the single source of truth for the vocabulary. The plugin adds these tags to a subchat atomically at `createChat` time:

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
- Tool-call failures never propagate to the dispatcher: validation errors and mid-spawn server failures are answered to the caller as `rhd_sub_chat error: …` (a failed answer is only logged); a failing duplicate-answer check skips the call entirely rather than risk a second answer.
- A `paused` subchat whose queue no longer matches the stored tool-call arguments (human tampering) is never reconciled or activated — the call is errored and the chat stays paused for an operator or Phase 6 recovery.
- The `on_chat_state_change` callback body is `tokio::spawn`ed — never `await`ed inline in the monitor's dispatch path (see the "Event Callbacks Must Not Block the WebSocket Read Task" pattern in `memory/development.md`).

## Testing

```bash
cargo test -p rhd_plugin_sub_chat
```

Unit tests cover the tag vocabulary, the embedded tool definitions, spawn-request parsing/validation, title generation, the golden async-notice string, and queue-prefix reconciliation. Integration tests verify live custom-event acknowledgment, per-chat registration of all three tools, and — against a bare chat server with `handler::handle_spawn` driven directly (assistant-with-tool_calls messages cannot be posted via `addMessage`) — the full async spawn: atomic tag set, queued seed order, activation, the exact background notice, idempotent re-runs, the sync parking seam with link-tag reuse, and validation errors answered without any chat creation.

## License

Part of the RHD project.
