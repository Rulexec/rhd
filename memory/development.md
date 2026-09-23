# Development Practices

## Planning

- Implementation plans saved to `plans/` folder as markdown files
- Plan naming: descriptive file names per [MEMORY.md](MEMORY.md) conventions, e.g. `<feature>-plan.md`
- Plans should include: goal, architecture, implementation steps, file changes, risks, success criteria

## Code Organization

- All crates prefixed with `rhd_`
- Shared dependencies managed in workspace root `Cargo.toml`
- Strict YAML parsing with `deny_unknown_fields` (chat server args, plugin configs)
- Field names use camelCase in YAML/JSON, snake_case in Rust structs (via `#[serde(rename_all = "camelCase")]`)

### File Size Limits

**Keep files under 500 lines.** When a file approaches or exceeds this limit, split it into smaller, logical modules. This applies to all source files and test files.

To check for large files:
- `mise run check-large-files` — checks backend files
- `mise run top-files-backend` — shows largest backend files

When splitting, extract test modules first, then split by logical responsibility. See `.agents/skills/code-splitting/SKILL.md` for detailed patterns and examples.

## Testing

**Prefer running tests via mise** — commands defined in `mise.toml` at project root.

### Mise Test Commands

| Command | Description |
|---------|-------------|
| `mise run test-cargo` | Cargo unit tests (`cargo test`) |
| `mise run test-all` | All tests (cargo) |

### Mise Check Commands

| Command | Description |
|---------|-------------|
| `mise run check-cargo` | Rust compilation check (`cargo check`) |
| `mise run check` | Cargo check |

### Backend E2E Tests

See [backend-e2e.md](backend-e2e.md)

## Build & Validation

- `cargo build` for compilation
- `cargo test` for unit tests
- Plugin configs are validated at startup (model map, `default` model, credentials must resolve)

## Committing

- Commit messages should be short and descriptive, inferred from the work completed
- Format: lowercase, no period, concise summary of changes
- Examples: "add seeded rng for e2e tests", "fix stream subscription race", "handle plugin disconnect in status tracker"
- Always use `git add -A` to stage all changes before committing

## Error Handling

- Chat server stays alive on per-connection and per-plugin errors
- All errors include context (file path, chat id, plugin id)
- Plugin config validation fails fast (missing `default` model, unresolved alias, missing credential)

## Plugin Development

### Pattern: Event Callbacks Must Not Block the WebSocket Read Task

**Context:** Writing or modifying event dispatch in `rhd_chat_client` (`client.rs`), or any plugin event handler that makes a client request (`add_message`, `ack_custom_event`, etc.)
**Rule:** Never `await` an event subscription callback inline in the WebSocket read task. Spawn the callback with `tokio::spawn` so the read task keeps processing incoming frames.
**Why:** An inline-awaited callback that issues its own request deadlocks: the request's response arrives at the WebSocket, but the read task is blocked waiting for the callback to finish, so the response is never processed. This was the root cause of the `rhd_plugin_todo_list` hang during `ai_completions:preRequest` handling (the plugin's `add_message` never completed and the AI request timed out waiting for acks).
**Example:**
```rust
// BAD: blocks the read task
(sub.callback)(data.clone()).await;

// GOOD: matches the pattern used for messageAdded, chatCreated, etc.
let callback = sub.callback.clone();
tokio::spawn(async move { (callback)(data).await; });
```

### Pattern: Every Plugin Must Acknowledge Live Custom Events

**Context:** Writing or reviewing any plugin in `plugins/` that registers with the chat server.
**Rule:** A registered plugin must acknowledge **every** custom event it receives — subscribe via `on_custom_event` and ack unhandled events immediately, in addition to the startup `getPendingAcks` drain. A startup-only drain is NOT sufficient: events delivered while the plugin is running would never be acked.
**Why:** Senders (`ai_completions:preRequest`, `ai_completions:preDrainQueue`) wait for acks from every registered plugin in their `PluginsMonitor` snapshot; one silent plugin parks the chat for the 30 s timeout and then errors it (`ai_completions:error`). `rhd_plugin_choice` shipped with no `on_custom_event` subscription and caused exactly this on the first `preDrainQueue` rollout; the guard test is `plugins/rhd_plugin_choice/tests/custom_event_ack_test.rs`.
**Example:** See "Acknowledging Unhandled Events" in [`plugins/README.md`](../plugins/README.md) and the always-ack subscriptions in `rhd_plugin_mcp` / `rhd_plugin_choice` / `rhd_plugin_commands`.

### Pattern: Plugins Are Event-Driven, Not Polling

**Context:** Implementing or modifying any plugin in `plugins/`
**Rule:** React to chat state changes via `ChatMonitor` callbacks; do not add polling loops that re-check all chats on a timer. Keep a one-time startup reconciliation pass for crash/pre-startup state.
**Why:** Polling is O(n) per tick and adds up to a second of latency; it does not scale with chat count. Canonical guidelines with code examples live in [`plugins/README.md`](../plugins/README.md).
**Example:** See "Event-Driven Plugin Design" in [features/plugins.md](features/plugins.md).

### Pattern: Consume Plugin State With Version Gating

**Context:** Subscribing to any well-known plugin state schema (`mcpStatus:1`, `errors:1`, …) from a plugin or the frontend.
**Rule:** `getPluginStates` → `subscribePluginStates` with the versions held (`0` = send latest) → apply only strictly newer versions; ignore older/equal events. Removals (`pluginStateRemoved`) are gated the same way.
**Why:** The server broadcasts all state changes unfiltered; catch-up responses and live events can overlap. Version gating makes duplicates and out-of-order delivery harmless.
**Example:** See "Plugin State" in [features/plugins.md](features/plugins.md) and "Plugin State" in [`plugins/README.md`](../plugins/README.md).

### Pattern: Plugin README Maintenance

**Context:** When implementing or modifying any plugin in the `plugins/` directory
**Rule:**
- When **implementing or changing** an existing plugin: maintain (update) the plugin's own `README.md` file to reflect the changes
- When **developing a new** plugin: consult `plugins/README.md` first for plugin development instructions and conventions

**Why:** Each plugin should have up-to-date documentation describing its purpose, configuration, and usage. The central `plugins/README.md` contains development guidelines that ensure consistency across all plugins.

**Example workflow:**
1. Starting new plugin work → Read `plugins/README.md` for development guidelines
2. Implementing feature X in plugin Y → Update `plugins/plugin_y/README.md` to document feature X
3. Changing plugin configuration → Update the plugin's README with new config options
