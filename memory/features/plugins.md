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

**Queued Message Processing:**
- Fetches queued messages from the chat
- Deletes each queued message from the queue
- Adds each as a regular message to maintain chat history
- Builds AI request from message history (filtering out error messages)

**Chat Tags:**
- `ai_completions:running` — an AI request is actively in progress (including between tool-loop iterations). Added right before the plugin acknowledges its own `preRequest` event; removed when the request completes without tool calls or when the chat transitions to `ai_completions:error`. Provides visibility so other plugins/UI can show processing state and avoid conflicting operations.
- `ai_completions:error` — the chat is parked after a failure; removed manually by an operator.

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

### Future Plugin Ideas

- **Notification Plugin**: Send notifications when specific events occur
- **Logging Plugin**: Log all chat activity to external systems
- **Moderation Plugin**: Filter or flag inappropriate content
- **Translation Plugin**: Translate messages between languages
