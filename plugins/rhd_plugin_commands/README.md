# rhd_plugin_commands

A plugin for the RHD chat system that executes slash-commands embedded at the
start of queued user messages just before the AI completions plugin promotes
them into the conversation, rewriting the queue accordingly.

## Overview

When a user queues a message like `/tags_example /prompt_example hello`, the
plugin — triggered right before the queue drain — applies the configured
effects of each recognized command (`/tags_example` updates chat tags,
`/prompt_example` inserts a prompt file before the message), strips the
executed commands from the message, and leaves the remaining text (`hello`) to
drain normally. Commands can also expand into multi-step sequences.

## Trigger Conditions

The plugin is purely event-driven (no polling, no ChatMonitor):

- **`ai_completions:preDrainQueue`** — emitted by the AI completions plugin on
  a queuedMessages-triggered request, after `ai_completions:preRequest` acks
  and **before** the queue is drained into the conversation.

  Payload: top-level `chatId` (string id), `additional.triggerReason` =
  `"queuedMessages"`. The plugin fetches the chat's queue via
  `getQueueMessages`, command-parses every `role == "user"` message snapshot,
  executes the recognized steps, finalizes the carrying messages, and then
  acknowledges the event.

- **All other custom events** are acknowledged unhandled (required so senders
  waiting for acks are never blocked).

- **Startup recovery**: `getPendingAcks` is processed before subscribing; any
  pending `preDrainQueue` events are handled like live ones. Re-parsing is
  safe: fully-stripped messages parse to no commands and pass through.

## Events Emitted

None.

## Commands

Configuration is a strict top-level `commands:` map (`deny_unknown_fields`).
Each value is one of the four shapes:

```yaml
commands:
  tags_example:              # single chat_tags step
    type: chat_tags
    add: [mcp:common]
    remove: [pause]
  prompt_example: ./commands/prompt.md   # shorthand: prompt with role=user
  system_prompt_example:     # explicit prompt step with a role
    type: prompt
    role: system
    prompt: ./systemPrompts/warhammer.md
  multi_example:             # ordered multi-step command
    - type: message_tags
      add: [some_tag]
    - ./commands/prompt.md
    - ./commands/another_prompt.md
```

Step effects (executed per occurrence, in order):

| type            | Effect |
|-----------------|--------|
| `chat_tags`     | `updateChat(chatId, addTags, removeTags)` — additive/subtractive, idempotent |
| `message_tags`  | `updateQueueMessage(messageId, addTags, removeTags)` on the carrying message |
| `prompt`        | `addQueueMessage(chatId, role, content, tags=["commands:prompt:<name>"], beforeMessageId=<carrying message id>)` — inserted directly before the command message |

Validation at startup (all failures abort with a clear error):

- Command-name keys must match `^[A-Za-z0-9_]+$` (the parser's name charset).
- Prompt paths are resolved relative to the config file's directory (absolute
  paths used as-is); files must exist and are read verbatim **once** at startup
  and cached.
- Prompt roles are restricted to `user` (default) / `system` / `assistant`;
  `tool` is rejected (tool messages require a `toolCallId`).
- `chat_tags` / `message_tags` with neither `add` nor `remove` is an error.

## Message Syntax

- Commands are only recognized at the **start** of the message (leading
  whitespace is skipped). Anything after the first non-command token is plain
  text.
- Whitespace between commands is optional: `/a/b` and `/a /b` are equivalent.
- A command name is the longest `[A-Za-z0-9_]` run after `/`; any other
  character terminates the name.
- An unknown `/token` **stops parsing**: its raw text and everything after it
  stay verbatim as the message content; commands before it still execute. If
  the very first token is unknown, the message is left byte-for-byte untouched.
- After all recognized commands ran: if the remainder is empty or
  whitespace-only the message is **deleted** from the queue, otherwise its
  content is updated to the (trimmed) remainder.
- Only `role == "user"` queued messages are scanned. Prompt messages inserted
  during the same pass are never re-parsed (no recursion), and assistant/tool
  messages pass through untouched.

## Examples

With the example configuration above (`config.example.yaml`):

- `/prompt_example hello` → the content of `commands/prompt.md` is inserted
  into the queue **directly before** the command message (tagged
  `commands:prompt:prompt_example`); the message becomes `hello`.
- `/tags_example /prompt_example` → chat gains `mcp:common` and loses `pause`;
  the prompt is inserted before the message; the message is removed (its
  content became empty).
- `/multi_example` → on the carrying message: tag `some_tag` added, then
  `commands/prompt.md` and `commands/another_prompt.md` inserted before it in
  config order; the message itself is consumed.
- `/notacommand stays` → unrecognized first token: no commands executed, the
  message drains verbatim.

## Tags Added

- `commands:prompt:<name>` — on every inserted prompt queue message; survives
  the drain into regular history (observability/dedup).
- Chat-tag and message-tag effects come entirely from the configuration (no
  fixed set). Tag namespaces are operator-managed on purpose: the plugin is a
  coordination surface, not a policy engine.

## Running

```bash
./rhd_plugin_commands --server-url ws://localhost:8080/ --config commands-config.yaml
```

### Command-Line Arguments

- `--server-url`: WebSocket URL of the chat server (required)
- `--config`: Path to the commands YAML file (required)
- `--plugin-id`: Plugin ID (optional, defaults to `commands`)

## Error Handling

- **Bad config = fail fast at startup** (unknown fields, invalid names/roles,
  missing prompt files); the plugin never runs with a half-loaded registry.
- **Runtime**: per-step client errors are logged with chat/message/command
  context and the remaining messages still get processed.
- **The event is always acknowledged**, even when processing fails — failures
  degrade to "commands skipped", never "AI request blocked". A missed or late
  ack would park every affected chat for 30 s and then error it.
- **Known limitation (crash mid-execution)**: recovery via `getPendingAcks`
  re-parses the queue; a partially applied message may have its remaining
  (unstripped) commands executed again, duplicating a prompt insert. The
  window is sub-second and matches the drain's own non-transactional nature.

## Dependencies

- **rhd_chat_client / rhd_chat_api**: server connection and queue operations
  (requires the positional `addQueueMessage` `beforeMessageId` support).
- **rhd_plugin_ai_completions**: emits the `ai_completions:preDrainQueue`
  event this plugin reacts to. Without the commands plugin running, the event
  is acked unhandled by other plugins and the queue drains unmodified.

## Manual Verification

```bash
# start server + ai_completions + commands plugin (config.example.yaml), then:
rhd queue add <chat> "/tags_example /prompt_example hello"
rhd queue add <chat> "/multi_example"
rhd queue add <chat> "/notacommand stays"
# expect per the milestone plan; check chat tags via `rhd chats` and history
# via `rhd messages <chat>`
```

## Development

### Running Tests

```bash
cargo test -p rhd_plugin_commands
```

### Code Structure

- `src/main.rs`: Entry point with CLI argument parsing
- `src/lib.rs`: Module exports, event-name constant, `prompt_tag()`
- `src/config.rs`: Strict config model, startup validation, prompt cache
- `src/parser.rs`: Pure leading-command parser
- `src/executor.rs`: Queue snapshot processing, step application, finalization
- `src/plugin.rs`: Plugin lifecycle and custom-event handling

## License

This project is part of the RHD workspace.
