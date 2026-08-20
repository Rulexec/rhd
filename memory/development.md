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
| `mise run test-e2e` | Backend e2e tests (`cargo build && cargo run -p rhd_test`) |
| `mise run test-all` | All tests (cargo + e2e) |

Pass arguments to rhd_test: `mise run test-e2e -- --seed 100 --repetitions 5`

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
