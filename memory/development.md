# Development Practices

## Planning

- Implementation plans saved to `plans/` folder as markdown files
- Plan naming: `<feature>-plan.md` or `<feature>-plan-<n>.md` for iterations
- Plans should include: goal, architecture, implementation steps, file changes, risks, success criteria

## Code Organization

- All crates prefixed with `rhd_`
- Shared dependencies managed in workspace root `Cargo.toml`
- Strict YAML parsing with `deny_unknown_fields`
- Field names use camelCase in YAML, snake_case in Rust structs (via `#[serde(rename_all = "camelCase")]`)

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
- Daemon validates models and scenarios at startup

## Committing

- Commit messages should be short and descriptive, inferred from the work completed
- Format: lowercase, no period, concise summary of changes
- Examples: "add seeded rng for e2e tests", "fix placeholder resolution bug", "update daemon shutdown logic"
- Always use `git add -A` to stage all changes before committing

## Important Conventions

1. **Placeholder resolution**: Missing values resolve to empty string, not errors. Exception: flag placeholders (`%step.flag_name%`) resolve to "false" when flag not set
2. **runCommand behavior**: Captures exit code + stdout/stderr, never fails scenario
3. **aiChat behavior**: Resolves placeholders in systemPrompt and message before API call
4. **output behavior**: Resolves placeholders in template, returns final string
5. **Model loading**: Filename (without extension) becomes model name in HashMap
6. **Scenario loading**: Directory name is scenario identifier (used as HashMap key), `scenario.yaml` contains definition
7. **Socket cleanup**: Daemon removes stale socket file on startup
8. **Graceful shutdown**: Daemon handles SIGTERM/SIGINT for clean shutdown
9. **CWD propagation**: `rhd run` sends its cwd to daemon; commands execute in client's cwd unless overridden in scenario
10. **Socket path**: Default socket location is `$HOME/rhd.sock`; both daemon and client accept `--socket` flag for custom location

## Error Handling

- Daemon stays alive on scenario errors
- Client exits with code 0 on success, 1 on error, 2 on abort
- All errors include context (file path, line number, step name)
- Model validation at daemon startup (exits if invalid)

## Scenario ID Persistence

- Scenario execution IDs are persisted in SQLite database to survive daemon restarts
- Database location: `<dbDir>/meta.db` (default: `rhd_db/meta.db`)
- Single table `meta` with column `nextScenarioId` (INTEGER)
- IDs are atomically incremented using SQLite transactions
- WAL mode enabled for better concurrency and crash recovery
- Database directory is created automatically if it doesn't exist
- Fails fast if database cannot be opened or accessed

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

### Pattern: Plugins Are Event-Driven, Not Polling

**Context:** Implementing or modifying any plugin in `plugins/`
**Rule:** React to chat state changes via `ChatMonitor` callbacks; do not add polling loops that re-check all chats on a timer. Keep a one-time startup reconciliation pass for crash/pre-startup state.
**Why:** Polling is O(n) per tick and adds up to a second of latency; it does not scale with chat count. Canonical guidelines with code examples live in [`plugins/README.md`](../plugins/README.md).
**Example:** See "Event-Driven Plugin Design" in [features/plugins.md](features/plugins.md).

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
