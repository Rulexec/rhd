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

## Testing

**Prefer running tests via mise** — commands defined in `mise.toml` at project root.

### Mise Test Commands

| Command | Description |
|---------|-------------|
| `mise run test-frontend-unit` | Frontend unit tests (vitest, no daemon) |
| `mise run test-frontend-e2e` | Frontend e2e tests (vitest, spawns daemon) |
| `mise run test-cargo` | Cargo unit tests (`cargo test`) |
| `mise run test-e2e` | Backend e2e tests (`cargo build && cargo run -p rhd_test`) |
| `mise run test-all` | All tests above |

Pass arguments to rhd_test: `mise run test-e2e -- --seed 100 --repetitions 5`

### Frontend Tests

Frontend tests split into two categories:

**Unit tests** — pure component/utils tests, no daemon spawn. Co-located with source files (e.g., `src/lib/utils.test.ts`). Config: `frontend/vitest.config.unit.ts`.

**E2E tests** — spawn daemon via `rhd_test frontend`. Located in `frontend/src/tests/e2e/`. Config: `frontend/vitest.config.e2e.ts`.

E2E test infrastructure:
- Tests spawn `rhd_test frontend` which starts:
  - Mock AI server on random port
  - Control HTTP server on random port (for test coordination)
  - rhd daemon with WebSocket server on random port
- Test utilities in `frontend/src/tests/testUtils.ts`:
  - `waitForWebSocket()` - waits for daemon to be ready
  - `configureMock(content)` - sets mock AI response
  - `getRecordedRequests()` - fetches recorded AI requests
- Tests use `@testing-library/svelte` for component rendering and interaction
- WebSocket port is dynamically set via `setWsPort()` and `connectWebSocket()` from `src/lib/ws.ts`
- **Known issue**: Chat UI does not auto-select first model when creating new chat (test works around this)

### Backend E2E Tests

- E2E tests via `rhd_test` crate
- `rhd_test` accepts `--seed` (default 42) for deterministic random generation and `--repetitions` (default 10) to run tests in loop
- Each iteration uses seed `base_seed + i`, prints iteration seed for reproducibility on failure
- `rhd_test` starts a mock OpenAI-compatible HTTP server (axum, reused across iterations), spawns daemon per iteration, runs scenario, validates AI request payloads and output
- Test scenarios in `test_e2e/scenarios/<name>/scenario.yaml`
- Test models in `test_e2e/models/*.yaml`

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
