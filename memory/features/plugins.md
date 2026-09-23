# Plugins Feature

## Overview

Plugins are external applications that extend the RHD chat system's functionality. They connect to the chat server via WebSocket, register themselves, and react to events in the system.

## What Plugins Can Do

- **Monitor Chats**: Track chat activity and respond to messages
- **Automate Tasks**: Perform automated actions based on chat events
- **Integrate External Services**: Connect to AI providers, databases, or other services
- **Add Custom Logic**: Implement custom business logic for chat processing

## How Plugins Work

### Plugin Lifecycle

1. **Connect**: Plugin connects to chat server via WebSocket
2. **Register**: Plugin registers with a unique ID using `registerPlugin` method
3. **Recover**: Plugin calls `getPendingAcks` to fetch events it may have missed while disconnected
4. **Subscribe**: Plugin subscribes to relevant events (chat list, individual chats, plugins list)
5. **React**: Plugin responds to events based on its logic

### Plugin Monitoring

Plugins can use built-in monitors from `rhd_chat_client`:

- **PluginsMonitor**: Tracks all registered plugins (active and inactive), manages custom event acknowledgments, provides wait logic for coordination. Acknowledgment recording is wired automatically when the monitor is created — plugins do not need to subscribe to `customEventAcknowledged` themselves.
- **ChatMonitor**: Tracks chats and their state (messages, queue count, tags), detects trigger conditions, provides methods to query chat state. Notifies registered callbacks whenever a chat's state changes, so plugins can react to events instead of polling.

These monitors are reusable by any plugin and handle subscription management automatically.

Additional client capabilities for plugins:

- **Tool call subscriptions**: Subscribe to assistant messages containing tool calls, filtered by tool name. Subscribing with `chat_id = 0` means "all chats" (wildcard).
- **Filtered message queries**: Query a chat's messages with filters (e.g. only messages with unresolved tool calls, or messages carrying specific tags) plus the chat version, without loading the full history.

### Event-Driven Plugin Design

Plugins react to chat state changes via callbacks instead of polling all chats on a timer:

- Trigger conditions are checked only for chats that actually changed — reaction latency is milliseconds, not seconds, and cost does not grow with chat count.
- **Startup reconciliation is still required**: once at startup, plugins check all monitored chats to handle chats that existed before the plugin started and to recover from crashes/inconsistent states.
- The canonical guidelines (recommended pattern, anti-patterns) live in [`plugins/README.md`](../../plugins/README.md).

### Custom Event Coordination

Plugins can coordinate their actions using custom events:

1. Plugin A sends a custom event (e.g., `ai_completions:preRequest`)
2. All connected plugins receive the event
3. Other plugins acknowledge the event using `ackCustomEvent`
4. Plugin A waits for all acknowledgments before proceeding
5. If a plugin doesn't acknowledge, the request times out

This mechanism ensures plugins can coordinate their actions and avoid conflicts. The `getPendingAcks` method returns all events not yet acknowledged by the calling plugin.

**Rejection:** a plugin can acknowledge an event with `is_rejected: true` to signal it refuses to participate. Rejection by one plugin does **not** prevent other plugins from acknowledging normally; the initiator receives each acknowledgment (accepted or rejected) with the correct `is_rejected` value.

**Context fields:** custom events can carry optional `chat_id`, `message_id`, and `tool_call_id` context fields identifying where the event originated. These fields are broadcast to subscribers, stored, and returned by `getPendingAcks` so plugins can reconstruct full event context after reconnection. Senders should use the dedicated top-level `chat_id` field rather than embedding the id in the `additional` JSON payload.

### Plugin State

Plugins can expose structured state so other plugins and the UI can see "here is my current situation" without asking:

- A state is a named value keyed by `(pluginId, key)` — the key is namespaced by the owning plugin, so two plugins can both publish `status` without colliding.
- Each state carries `content` in a declared `format` (`markdown` or `json`) plus a `schema` string naming a well-known content format and its version (e.g. `mcpStatus:1`, `errors:1`). Consumers recognize states by `schema` and ignore unknown schemas/versions.

**Lifecycle:**

- Plugins create or replace states with `updatePluginState` and delete them with `removePluginState`.
- States **persist across plugin disconnects**: an inactive plugin's states remain visible as "last known" until the plugin reconnects and updates them.
- States are deleted together with the plugin (`removePlugin`).

**Versioning contract** (consumer-facing): each state has a server-assigned version that starts at `1` and bumps on **every** update **and** on removal. Re-creating a removed state continues from the tombstone, so versions are monotonic per `(pluginId, key)` over its whole lifetime. Consumers must ignore any event whose version is not strictly newer than what they already hold.

**Race-free consumption recipe** (a naive get→subscribe can miss or duplicate updates):

1. `getPluginStates` — fetch the current states of interest.
2. `subscribePluginStates` passing the versions held from step 1 (or `0` for "send me the latest") — the server registers the subscription before snapshotting, so nothing slips through the gap.
3. Apply the catch-up response, then handle `pluginStateChanged` / `pluginStateRemoved` events version-gated (apply only strictly newer versions).

**UI behavior:**

- The Plugins tab lists each plugin's states in a collapsed "State (n)" section.
- The MCPs tab is shown only while some plugin publishes an `mcpStatus:1` state, and renders the per-server status from those states.

### Event Model

Plugins interact with the system through events:

- **Chat Events**: Message added, updated, deleted; queue message changes
- **System Events**: Chat created, updated, deleted; plugin registered, removed
- **Custom Events**: Plugins can emit and listen for custom events

### Acknowledgments

When a plugin sends a custom event, other plugins can acknowledge it. This allows coordination between plugins:

1. Plugin A sends custom event
2. Plugin B and C receive the event
3. Plugin B and C acknowledge the event
4. Plugin A waits for all acknowledgments before proceeding

This mechanism ensures plugins can coordinate their actions and avoid conflicts.

## Deploying Plugins

### Requirements

- Plugin must be a standalone binary
- Plugin must accept server URL and config file path as arguments
- Plugin must handle disconnections gracefully

### Configuration

Plugins typically use YAML configuration files with:
- Server connection details
- API keys and credentials
- Plugin-specific settings

Credentials should be stored in a separate file referenced by the main config.

### Running a Plugin

```bash
./my-plugin --server-url ws://localhost:8080/ --config plugin-config.yaml
```

### Monitoring Plugins

Use the chat server's plugin management API to:
- List active plugins
- Check plugin status
- View plugin events

## Error Handling

Plugins should handle errors gracefully:
- Log errors with context
- Retry transient failures
- Notify users of persistent failures
- Continue running even if individual operations fail

## Security Considerations

- Plugins run with the same permissions as the user who started them
- Plugins can access all chats and messages
- API keys should be stored securely
- Plugins should validate all inputs

## Example Plugins

### AI Completions Plugin

The `rhd_plugin_ai_completions` plugin is event-driven: it reacts to chat state changes and triggers AI completions when:
- There are queued messages and no unresolved tool calls
- All tool calls from the last assistant message are resolved (tool loop continuation)

**Trigger Detection:**
- Checks for queued messages in the chat
- Verifies no unresolved tool calls exist
- Skips chats with `ai_completions:error` tag
- **Guardrail:** at most one AI request is in flight per chat; a chat already being processed is skipped by subsequent triggers

**Pre-Request Coordination:**
- Emits `ai_completions:preRequest` custom event before making AI requests
- Waits for all other plugins to acknowledge the event
- Allows other plugins to modify state or block the request
- Emits `ai_completions:preDrainQueue` **only on the queuedMessages trigger**, after `preRequest` acks and immediately before queued messages are promoted into the conversation; waits for all other plugins to acknowledge so they can inspect/rewrite the queue at the last moment (a missed ack within the timeout parks the chat with `ai_completions:error`, same semantics as `preRequest`)

**Queued Message Processing:**
- Fetches queued messages from the chat
- Deletes each queued message from the queue
- Adds each as a regular message to maintain chat history (in queue position order)
- **Empty-drain skip:** if the drain promotes zero messages (a plugin emptied the whole queue during `preDrainQueue`, e.g. `rhd_plugin_commands` consuming a text-less command message) **and** no tool-loop continuation is owed (last assistant has no fully-resolved tool calls), the plugin does not send an AI request at all — it acknowledges its own `preRequest`, sets no `running` tag, creates no streaming message, and the chat simply waits for the next real queued message. A genuine pending continuation still fires despite the empty drain.
- Builds AI request from message history (filtering out error messages)

**Chat Tags:**
- `ai_completions:running` — an AI request is actively in progress (including between tool-loop iterations). Added right before the plugin acknowledges its own `preRequest` event; removed when the request completes without tool calls or when the chat transitions to `ai_completions:error`. Provides visibility so other plugins/UI can show processing state and avoid conflicting operations.
- `ai_completions:error` — the chat is parked after a failure; removed manually by an operator.
- `paused` — platform-level pause (not owned by this plugin): a chat carrying the exact tag never triggers, for either queued messages or tool-loop continuation; removing it resumes the normal flow on the chat's next state change. Requests already in flight are unaffected — the current one completes and only the next trigger is blocked. Any chat can be paused manually from the UI; the sub-chat plugin uses it to stage new subchats (see Sub Chat Plugin below).

**Startup Reconciliation:** once at startup the plugin fixes chats left inconsistent by a crash:
- Chat with `ai_completions:running` and an unfinished message → park with `ai_completions:error`, remove `running`
- Chat with `ai_completions:running` but eligible for a request → remove `running` only, normal flow continues
- Chat with an unfinished message and no `running` tag → park with `ai_completions:error`

**Error Handling:**
- On AI request failure: adds `ai_completions:error` tag to chat (and removes `ai_completions:running`)
- Adds error message tagged `ai_completions:error` containing error details
- Skips chats with error tag (no further processing)
- Filters out error messages when building AI requests

**Tool Call Handling:**
- If AI response contains tool calls, adds assistant message with tool calls to chat
- Other plugins execute the tools and add results as tool messages
- Plugin detects when all tool calls are resolved and continues the loop
- Tools registered for the chat (via `addTools`) are passed to the AI provider in the request

**Request Fidelity ("complete or not at all"):** the request sent to the provider is a faithful rendering of stored history — a request is never sent with data quietly stripped:
- Assistant `tool_calls` and `reasoning_content` are passed through to the provider
- `tool`-role messages carry `tool_call_id` end-to-end, so tool results are replayed correctly
- A chat with unresolved tool calls never triggers a request; it triggers as soon as every tool call id is answered
- If the stored history is inconsistent (orphaned/missing tool-call data), the request is refused and the chat is parked with `ai_completions:error` for a human to inspect
- Messages with unknown roles and `ai_completions:error`-tagged messages are excluded by policy (never coerced into user messages)
- `reasoning_content` can be suppressed per model via `sendReasoningContent: false` in the plugin config (some providers reject it)
- Repairing chats parked by a crash is not yet implemented — a parked chat stays parked until an operator clears the tag

### System Prompt Plugin

The `rhd_plugin_system_prompt` plugin injects system prompts into chats based on chat tags.

**Configuration:**
- YAML config maps prompt names to file paths under a `systemPrompts` key
- Relative paths resolve against the config file's directory; absolute paths are used as-is
- All prompt files are read once and cached at startup; a missing/unreadable file fails startup with a clear error

**Injection Behavior:**
- A chat tagged `systemPrompt:<name>` should have the prompt `<name>` present as a system message; injected messages receive the same `systemPrompt:<name>` tag for duplicate detection
- Multiple prompts per chat are supported (multiple tags)
- Prompts added to a chat after startup are picked up automatically
- Injection never happens while a chat has an unfinished assistant message

**Coordination with AI Completions:**
- Listens for `ai_completions:preRequest` events; if a required prompt is missing, injects it **before** acknowledging, so the prompt is in place when the AI request is built
- The event is **always** acknowledged (even when injection is skipped or fails) — never rejected or dropped — otherwise the AI request times out waiting for acks

**Gating by AI Completions Tags:**
- A chat carrying `ai_completions:running` or `ai_completions:error` is locked: no injection while the AI plugin owns the chat or the chat is parked
- Effect: prompts are injected only when the chat is new or awaits its next queued message. A prompt required mid-tool-loop is deferred and injected within one tick after the chat returns to idle (or after an operator clears the error tag)
- Troubleshooting: if prompts are not being injected, check the chat does not carry an `ai_completions:running`/`ai_completions:error` tag

### Commands Plugin

The `rhd_plugin_commands` plugin turns leading slash-commands in a queued message into actions on the queue itself, so a user can enrich a message ("apply this prompt, set these tags") before it reaches the model.

**What the user does:** at the start of a queued message they type one or more `/name` commands, optionally followed by the actual text. The command tokens run and are stripped; what remains is the message the model sees. Examples: `/prompt_example hello` inserts a prompt just before the message and leaves `hello`; `/tags_example /prompt_example` (commands only) applies the tags, inserts the prompt, and the now-empty message is removed.

**Syntax rules:**
- Only the **leading** run of commands counts — the message must start with `/` after any whitespace.
- Commands may be whitespace-separated (`/a /b`) or adjacent (`/a/b`); they execute left to right.
- A **multi-command** expands into an ordered list of steps (tags + one or more prompts) executed in config order.
- An **unknown** `/token` (or `/` with nothing after it) is not a command: parsing stops and its text — plus everything after — is kept **verbatim** as the message content. A message whose first token is unknown is left untouched.

**Command effects:** apply chat tags (add/remove), apply message tags to the carrying message, or insert a prompt message. A prompt is inserted **directly before** the command message (not appended), so multiple queued command messages keep their interleaved order when promoted. Inserted prompts carry a `commands:prompt:<name>` tag for observability. Prompt file contents are read once and cached at startup; a missing prompt fails startup.

**Coordination & safety:**
- Reacts to `ai_completions:preDrainQueue` (fires only on the queuedMessages trigger). It **always acknowledges** that event — even when a message's commands fail — so one bad message never parks the chat; other plugins that don't handle the event acknowledge it automatically, so the queue drains normally with the plugin down.
- Only `user`-role queued messages are scanned; assistant/tool messages and messages with a `tool_call_id` are never rewritten.
- **A text-less command never wakes the model:** a command-only message whose steps consume it (e.g. a pure tag command like `/mcp_on`) is removed from the queue, and when that empties the queue the AI completions plugin's empty-drain skip means no request is sent — the command applies and the chat waits for the next real message. Commands that leave text (or insert a prompt) still drain and trigger normally.
- **Crash window:** if the plugin dies between inserting a prompt and updating the carrying message, a startup recovery re-processes the pending event; because already-stripped commands are re-parsed as plain text this is safe, but a partially-applied message could re-insert a prompt (duplication). The window is sub-second and is a documented limitation.
- Because the queue's internal order is not exposed on the wire, a prompt can briefly render at the queue end in the live view until the drain (milliseconds); a reload shows the final order.

### Tool Call Tags

Plugins can attach tags to individual tool calls stored on assistant messages via `updateToolCallTags` (add/remove tag arrays on a specific tool call of a message):

- Tags are per tool call, not per message
- Changes bump the chat version and broadcast a `messageUpdated` event carrying the updated tool calls and `chatVersion`, so watching plugins can validate they hold the latest state
- Tool-call tags are RHD-internal orchestration metadata — they are never sent to the AI provider
- Tag mutations must go through `updateToolCallTags`; the plugin writes `toolCalls` only once at stream finish (a later full rewrite of the tool-calls blob would wipe tags)

### Todo List Plugin

The `rhd_plugin_todo_list` plugin provides structured task tracking for multi-step operations in chat conversations.

**Automatic Initialization:**
- Detects new chats via chat state change events
- Checks if contract system message already exists (tagged with `todo_list:contract`)
- If not, adds a system message with the tool contract and registers the `rhd_set_todo_list` tool

**Tool Call Handling:**
- Subscribes to `assistantMessageWithToolCalls` events filtered by tool name `rhd_set_todo_list`
- Parses the `todos` parameter as a markdown checklist
- Validates format and stores the todo list per chat
- Returns success message or error with format example

**AI Context Injection:**
- Subscribes to `ai_completions:preRequest` custom events
- Retrieves current todo list for the chat
- Injects a system message with the todo list (or empty template if no items)
- Acknowledges the event to allow AI request to proceed

**Todo List Format:**
```markdown
[ ] Pending task
[-] In progress task
[x] Completed task
[!] Discarded task
```

**Error Handling:**
- If todo list format is invalid, returns error message with correct format example
- Checks for duplicate tool call results to avoid processing the same call twice

### MCP Plugin

The `rhd_plugin_mcp` plugin exposes external MCP servers as chat tools.

**Configuration:**
- YAML `mcp:` list: `id` (defaults to `name`), `name`, `cmd`, `args`
  (literals or `env: VAR` resolved from the plugin environment), `cwd`
  (default: plugin's working directory), `env` map for the server process,
  optional `registerOnTag`.

**Behavior:**
- Spawns configured servers at startup best-effort: a server that fails to start is reported as `error` in the plugin's `mcpStatus:1` state while the healthy servers keep working; config parse errors remain fatal.
- Reports per-server run status via the `mcpStatus:1` plugin state (see [mcp-plugin.md](mcp-plugin.md)).
- Registers tools on eligible chats prefixed `<name>:` (e.g. `filesystem:read_file`).
- Gating: with `--worktree W`, only chats tagged `worktree:W`; without it,
  only chats with no `worktree:*` tag. `registerOnTag` additionally requires
  that exact chat tag. Registration is additive per (chat, server).
- Subscribes to its tool calls, executes them on the owning server
  (serialized per server), and answers with `tool`-role messages carrying
  `toolCallId`; duplicate calls are skipped; MCP errors become tool content.
- Acknowledges all custom events (handles none).

See [mcp-plugin.md](mcp-plugin.md) for the product view.

### Choice Plugin

The `rhd_plugin_choice` plugin lets the assistant ask the user to choose between concrete options before continuing — the decision belongs to the human, not the model.

**What the user sees:**
- The choice tool is available in every chat without any user setup.
- When the assistant decides the next step depends on a user decision, the chat shows a question card: the question text, one button per offered option, and a free-text input for an answer that is none of the options.

**How answering works:**
- Clicking an option or submitting typed text is a human UI action — the frontend sends the chosen or typed text back as the answer to the assistant's question on the user's behalf.
- Once answered, the card switches to a resolved state showing the answer; reloading the page keeps it resolved (derived from the chat history).

**Pausing the assistant:**
- An unanswered choice intentionally pauses the assistant's tool loop: the conversation waits until the user decides. That is the point of the feature.

### Sub Chat Plugin

The `rhd_plugin_sub_chat` plugin lets the assistant delegate a self-contained task to a fresh **subchat** — a regular new chat that runs with its own history and the full pipeline (system prompts, MCP tools, todo contracts, AI completions) and finishes with a final assistant answer. Subchats appear in the UI as ordinary chats carrying tags; there is no special presentation.

**Tools (registered in every chat, including subchats — so a subchat can spawn its own subchats):**
- `rhd_sub_chat` — spawns a subchat from ordered seed `messages` (roles restricted to `system` and `user`) plus optional extra `tags` (e.g. `systemPrompt:<name>`, `worktree:<id>`). Sync mode (default) returns only when the subchat is done, with its final assistant message as the result; `async: true` returns immediately with the subchat id, to be inspected later with the other two tools.
- `rhd_sub_chat_status` — answers exactly `pending` or `completed` for a direct-child subchat, evaluated fresh on every call (a completed-then-reactivated subchat correctly reads `pending` again).
- `rhd_sub_chat_await` — holds until a direct-child subchat completes, then returns its final assistant message content verbatim; an already-completed target is answered immediately.

**Answer contracts:**
- "Completed" = the subchat is idle with a final answer: its last message is a finished, non-streaming assistant message with no tool calls, its queue is empty, and it carries neither `ai_completions:running` nor `ai_completions:error`.
- A subchat parked on an error reads `pending` — and sync spawns/awaits on it stay parked — until an operator fixes the chat. There are no timeouts anywhere: un-sticking a hung subchat is deliberately an operator action, not a tool-side one.
- Scope rule: status/await only work on chats this chat spawned itself — grandchildren, siblings, and unrelated chats are refused with a tool error. Malformed or invalid spawn arguments are answered as descriptive tool errors with no chat created, so the model can retry.
- A mid-spawn failure is answered with an error naming the (still-paused) subchat id when one exists, leaving it intact for recovery or an operator to finish; nothing is silently half-started.

**Lineage tags:** a subchat is created with `parent:<spawnerId>`, `root:<topAncestorId>`, and `sub_chat:call:<toolCallId>` binding it to the spawning tool call. The `root:` tag is inherited down the whole chain (in a tree spawned from A, every descendant carries `root:<A>`, so one tag scan finds the tree), and a first-time spawner tags itself with its own root. These shapes plus the exact tag `paused` are reserved — user-supplied spawn tags colliding with them are rejected.

**Two-phase activation:** the subchat is created `paused` with its full tag set in one atomic step, its seed messages are queued in order only afterwards, and `paused` is removed last as the single activation point — the AI completions plugin never sees a half-populated subchat (see the `paused` tag in the AI Completions section).

**Zero private state:** the plugin persists nothing of its own. After any restart, unfinished work is reconstructed at startup from chat history — unresolved sub-chat tool calls plus the link tags say what was spawned or parked, and each call is re-driven through the same logic as the live path. No work is lost and no subchat call is ever answered twice.

### Future Plugin Ideas

- **Notification Plugin**: Send notifications when specific events occur
- **Logging Plugin**: Log all chat activity to external systems
- **Moderation Plugin**: Filter or flag inappropriate content
- **Translation Plugin**: Translate messages between languages
