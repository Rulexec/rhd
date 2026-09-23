# rhd_plugin_sub_chat

A plugin that provides the sub-chat delegation tools — `rhd_sub_chat`, `rhd_sub_chat_status`, `rhd_sub_chat_await` — to every chat in the RHD chat system.

## Overview

`rhd_plugin_sub_chat` lets an assistant delegate a self-contained task to a fresh **subchat**: a new chat that runs with its own history and tools and finishes with a final assistant answer. The plugin registers all three tools in every chat (choice-style per-chat registration), so a subchat can itself spawn subchats — recursion works without special-casing.

- `rhd_sub_chat` — spawn a subchat from ordered `system`/`user` seed messages, with optional extra tags; sync mode returns the subchat's final answer, `async=true` returns the new chat id immediately.
- `rhd_sub_chat_status` — `pending` / `completed` check of a direct-child subchat.
- `rhd_sub_chat_await` — hold the tool call until a direct-child subchat finishes, then return its final answer verbatim.

> **Status note.** All three tools are live. Spawn: async mode answers with the background notice, sync mode parks the caller's tool call until the completion watcher answers it with the subchat's final assistant message. Status/await (Phase 5): both enforce the direct-child rule over a fresh `getChat`, status answers the literal `pending` / `completed`, await answers the final content immediately when the subchat is already complete or registers a watcher waiter otherwise. **Startup recovery (Phase 6)** is live too: after any plugin/server restart, `recovery::run` re-discovers every unfinished sub-chat tool call from chat history alone and re-drives it through exactly the live-path code (see *Startup Recovery* below).

## Trigger Conditions

- On every chat state change (via `ChatMonitor`): if the three tools have not yet been registered for that chat, call `addTools` once per chat with all three definitions. Chats created while the plugin runs register immediately (`chatCreated`); pre-existing chats are picked up on their next state change.
- An in-memory set of initialized chats avoids redundant `addTools` requests. Server-side registration is idempotent (`INSERT OR REPLACE` keyed by chat + plugin + tool name), so plugin restarts and re-delivered events are harmless. On an `addTools` error the chat is deliberately not marked initialized, so the next state change retries registration.
- On `assistantMessageWithToolCalls` for any chat (`on_tool_call(0, …)`, subscribed once with all three tool names): each `rhd_sub_chat` call runs the spawn flow:
  1. **Claim & guard:** `tool_call_id` is first *claimed* in the `AnswerGuards` processing-claim set — a live handler or the startup scan already owning it makes the entry return silently (the owner answers/registers; an RAII guard releases on every exit path) — then the duplicate-answer guard: a `tool` message answering this `tool_call_id` already exists → skip (persisted second line of defense; `reply::has_tool_result`).
  2. **Validate:** roles restricted to `system`/`user`, non-empty `messages`, string contents, user tags free of reserved shapes — failures are answered as a single-line `rhd_sub_chat error: …` with **no chat created**.
  3. **Converge** (`spawn::converge_spawn`, the two-phase activation protocol and future recovery kernel): ensure the caller's `root:` tag (self-tag the caller when absent) → find the subchat by its `sub_chat:call:<toolCallId>` link tag or create it **in one atomic `createChat`** with the full tag set (`parent:`, `root:`, link, user tags, `paused`) → while still `paused`, reconcile the message queue against the plan prefix (a tampered queue → refuse, error the call, leave the subchat paused) and enqueue missing starting messages strictly in order → **remove `paused` last** (the activation point — `ai_completions` never sees a half-populated subchat).
  4. **Answer:** `async: true` → the fixed notice `Chat started in background with id <chatId>, use tools rhd_sub_chat_status or rhd_sub_chat_await on it`. Sync → registers a **waiter** on the subchat via `Watcher::register_or_complete` (the call stays parked until the subchat completes; the same call answers immediately if the subchat is *already* complete — closing the register-vs-completion race). Mid-spawn server failures are answered `rhd_sub_chat error: failed to prepare subchat <id|new>: <detail>`; with a known id the "(subchat left paused for recovery)" suffix is appended and the paused subchat is left intact.

- Each `rhd_sub_chat_status` / `rhd_sub_chat_await` call (same `assistantMessageWithToolCalls` dispatch) parses `{"chatId": <int>}` and passes the shared `ensure_direct_child` gate: a fresh `getChat` of the target, rejected with a single-line `{tool} error: …` answer when the chat does not exist (`chat <id> not found`), the fetch fails for another reason (`chat <id> lookup failed: <detail>`), or the target lacks the exact `parent:<callerChatId>` tag (`chat <id> is not a direct subchat of this chat`) — siblings, grandchildren, self, and unrelated chats are all refused (AD-5; cyclic awaits are structurally impossible). Malformed arguments are answered `{tool} error: invalid arguments: <detail>` before any lookup.
  - **Status** then answers the literal `pending` or `completed` — a fresh evaluation of the completion predicate on every call, never cached watcher state — with no reason suffix. A reactivated subchat correctly reads `pending` again.
  - **Await** on an already-completed target answers the final assistant `content` verbatim immediately, without touching the watcher; otherwise it registers a watcher waiter (indistinguishable from a sync-spawn waiter — the same drain answers both) and the call stays parked. No timeouts.
- On a **watched subchat's** state change (via the single `ChatMonitor` hook installed by `watcher::install`): if the subchat satisfies the completion predicate (`completion::is_completed` — last message is a finished, non-streaming assistant message with no tool calls, the queue is drained, and no `ai_completions:running` / `ai_completions:error` tag), **every** waiter registered on it is answered with that final assistant `content` verbatim, then atomically removed (take-before-answer, so concurrent events cannot double-fire). Failed answers (e.g. a deleted parent) are logged and the waiter dropped — never re-queued.

## Startup Recovery (Phase 6, AD-7)

The plugin keeps **zero private state**, so a restart loses only in-memory waiters and claims — all reconstructible. `recovery::run` executes once at startup, after the tool-call subscription and the watcher hook are installed, and its failure is **never fatal** (parked calls stay parked; the next restart re-drives them — every step is idempotent).

1. **Index:** one unfiltered `listChats` pass maps `tool_call_id → subchat` via the `sub_chat:call:<id>` link tags (duplicate links — pathological double-create — keep the lowest chat id and warn).
2. **Scan:** for every monitored chat, `getMessages(withUnresolvedToolCalls: true)` (the server computes "declared call lacking a persisted `tool` answer"), filtered to the three tool names. A vanished/unreadable chat is logged and skipped.
3. **Serialize:** each unfinished call is claimed in the shared `AnswerGuards` processing-claim set **outermost, exactly once**, then re-driven through the *same unclaimed flows* the live entries delegate to (`handler::flows::{spawn_flow, status_flow, await_flow}` — never re-claiming its own id), with the persisted `has_tool_result` check as the second line of defense. A failed claim means a live handler owns the call → skip.
4. **Re-drive:** `rhd_sub_chat` → `converge_spawn` (absent B: full create + seed + activate; paused B: reconcile the queue prefix and activate; tampered queue: error answer, B stays paused) → async: late background notice answer; sync: re-register the waiter. `rhd_sub_chat_status` / `rhd_sub_chat_await` → fresh direct-child revalidation, answer-or-re-register — the world may have changed while the plugin was down.
5. **Report:** a `RecoveryReport` (`scanned_calls` / `respawned` / `reconciled_paused` / `rewatched` / `answered` / `skipped_answered`) is logged; the exactly-once invariants are enforced by the guards, not the counters.

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
- A failed **startup recovery** pass is logged (`startup recovery failed — some subchat calls may stay parked`), never fatal: everything recovery does is idempotent, so the next restart finishes the job. Per-chat (`getMessages` on a vanished chat) and per-call (failed persisted-answer guard read) failures are logged and skipped inside the pass.
- Per-chat failures (tool-definition parse errors, `addTools` errors) are logged and the chat is left uninitialized so the next state change retries registration.
- Tool-call failures never propagate to the dispatcher: validation errors and mid-spawn server failures are answered to the caller as `rhd_sub_chat error: …`, status/await rejections as `rhd_sub_chat_{status,await} error: …` (a failed answer is only logged); a failing duplicate-answer check skips the call entirely rather than risk a second answer. Every rejected status/await call still gets its error answer, so the caller's tool loop never parks on a refusal.
- A `paused` subchat whose queue no longer matches the stored tool-call arguments (human tampering) is never reconciled or activated — the call is errored and the chat stays paused for an operator or Phase 6 recovery.
- The `on_chat_state_change` callback body is `tokio::spawn`ed — never `await`ed inline in the monitor's dispatch path (see the "Event Callbacks Must Not Block the WebSocket Read Task" pattern in `memory/development.md`).

## Testing

```bash
cargo test -p rhd_plugin_sub_chat
```

Unit tests cover the tag vocabulary, the embedded tool definitions, spawn-request parsing/validation, title generation, the golden async-notice string, queue-prefix reconciliation, the completion predicate (exhaustive table), the `getChat`→`ChatState` mapping, `chatId` argument parsing (`{"chatId":5}` accepted; missing key, non-integer values, non-object and unparseable JSON rejected), the processing-claim set (exclusivity, RAII release on drop, refused-claim no-ops), the recovery link-tag index (near-miss tags ignored, duplicate links keep the lowest chat id), and the recovery classification table (flow outcome × index entry → report counter). Integration tests verify live custom-event acknowledgment, per-chat registration of all three tools, and — against a bare chat server with `handler::handle_spawn` driven directly — the full async spawn (atomic tag set, queued seed order, activation, the exact background notice, idempotent re-runs, validation errors answered without any chat creation), the sync path (park-then-answer, negative intermediate states, byte-identical verbatim answers, duplicate-event safety, and multi-waiter broadcast with late-registration immediate answer via `register_or_complete`), and the status/await tools driven through the production `handler::handle_tool_call_event` dispatch (direct-child rule incl. foreign child / grandchild / self / unknown id, byte-exact error answers that always resolve the caller's loop, fresh `pending`↔`completed` flipping across message/queue edits, await fast path without watcher registration, and await slow path with two-waiter broadcast). The startup-recovery suite (`tests/recovery_test.rs`) constructs crash states directly in stored history — unresolved assistant calls are built via `addMessage` + `updateMessage(toolCalls)`, which the server accepts from a plain connection — and asserts all plan scenarios: partial paused queue reconciled in order and activated (call stays parked, waiter registered), absent-B full re-spawn ending in a verbatim answer through the live watcher, late async notice answered exactly once with the golden string, already-answered calls skipped untouched, await-across-restart answered immediately, tampered-queue refusal (error answer, B stays paused, foreign queue untouched), and a back-to-back second pass adding zero chats, messages and answers.

## License

Part of the RHD project.
