# Mise Test Commands Plan

## Goal

Create `mise.toml` with test commands for easy test execution, and split frontend tests into unit and e2e categories.

## Current State

- `mise.toml` exists but empty
- `frontend/package.json` has no test scripts
- `frontend/vite.config.js` has inline vitest config
- `frontend/src/tests/chat.test.ts` spawns daemon via `rhd_test frontend` (e2e behavior)
- `frontend/src/tests/setup.ts`, `testUtils.ts` — test utilities
- Backend: `cargo test` for unit tests, `cargo run -p rhd_test` for e2e tests (standard + mcp)
- `rhd_test frontend` spawns daemon for frontend e2e tests

## Test Categories

| Category | Command | Description |
|----------|---------|-------------|
| Frontend Unit | `mise run test-frontend-unit` | Vitest tests without daemon (pure component/utils) |
| Frontend E2E | `mise run test-frontend-e2e` | Vitest tests that spawn daemon via `rhd_test frontend` |
| Cargo Unit | `mise run test-cargo` | `cargo test` for all crate unit tests |
| Backend E2E | `mise run test-e2e` | `cargo build && cargo run -p rhd_test` (standard + mcp tests) |
| All Tests | `mise run test-all` | Runs all above commands |

## Implementation Steps

### 1. Split Frontend Test Structure

Unit tests co-located with source files (e.g., `src/lib/utils.test.ts` next to `utils.ts`).

E2E tests in separate directory:
```
frontend/src/tests/
├── e2e/            # Tests that spawn daemon
│   └── chat.test.ts  (move from src/tests/)
├── setup.ts        (shared setup)
└── testUtils.ts    (shared utilities)
```

### 2. Create Separate Vitest Configs

**`frontend/vitest.config.unit.ts`**:
- Include: `src/**/*.test.ts` excluding `src/tests/e2e/**`
- Environment: happy-dom
- Setup: `src/tests/setup.ts`

**`frontend/vitest.config.e2e.ts`**:
- Include: `src/tests/e2e/**/*.test.ts`
- Environment: happy-dom
- Setup: `src/tests/setup.ts`
- Longer timeout (daemon spawn)

**`frontend/vite.config.js`**: Keep as-is for dev server (remove inline test config or keep as default).

### 3. Add npm Scripts to `frontend/package.json`

```json
{
  "scripts": {
    "test:unit": "vitest run --config vitest.config.unit.ts",
    "test:e2e": "vitest run --config vitest.config.e2e.ts"
  }
}
```

### 4. Create `mise.toml` Tasks

```toml
[tasks]
# Frontend tests
test-frontend-unit = { cwd = "frontend", command = "npm", args = ["run", "test:unit"] }
test-frontend-e2e = { cwd = "frontend", command = "npm", args = ["run", "test:e2e"] }

# Backend tests
test-cargo = "cargo test"

[tasks.test-e2e]
run = ["cargo build", "cargo run -p rhd_test"]

# Run all tests
test-all = { depends = ["test-frontend-unit", "test-frontend-e2e", "test-cargo", "test-e2e"] }
```

## File Changes

| File | Action |
|------|--------|
| `frontend/src/tests/e2e/` | Create directory |
| `frontend/src/tests/chat.test.ts` | Move to `e2e/chat.test.ts` |
| `frontend/vitest.config.unit.ts` | Create |
| `frontend/vitest.config.e2e.ts` | Create |
| `frontend/package.json` | Add `test:unit`, `test:e2e` scripts |
| `mise.toml` | Add all test tasks |

## Risks

- Existing `chat.test.ts` imports may need path updates after move
- `testUtils.ts` imports in e2e tests need relative path adjustment
- Mise `depends` runs tasks in parallel — if sequential execution needed, use `run_task` instead

## Success Criteria

- `mise run test-frontend-unit` runs only unit tests (currently none, but infrastructure ready)
- `mise run test-frontend-e2e` runs `chat.test.ts` with daemon
- `mise run test-cargo` runs all cargo unit tests
- `mise run test-e2e` runs cargo build then rhd_test standard + mcp tests
- `mise run test-all` runs all test commands
