# Memory Reorganization Plan

## Goal

Reorganize `memory/` docs: extract E2E testing details from `development.md` into separate files, actualize `file-structure.md`, create debugging guide.

## Changes

### 1. Extract frontend E2E testing → `memory/frontend-e2e.md`

Move from [`development.md`](memory/development.md:32) lines 32-55 (Frontend Tests → E2E section) to new `memory/frontend-e2e.md`. Keep unit test mention in `development.md` (one-liner reference).

Content for `frontend-e2e.md`:
- Test infrastructure (rhd_test frontend, mock AI server, control server, daemon)
- Test utilities (`testUtils.ts` functions)
- Config: `vitest.config.e2e.ts`
- Known issues

### 2. Extract backend E2E testing → `memory/backend-e2e.md`

Move from [`development.md`](memory/development.md:57) lines 57-70 to new `memory/backend-e2e.md`.

Content for `backend-e2e.md`:
- rhd_test crate usage (--seed, --repetitions)
- Mock server architecture (StreamChunkSender, mpsc channel, control server endpoints)
- Test scenarios/models location
- SSE streaming details

### 3. Update `development.md`

Replace extracted sections with short references:
```markdown
### Testing
- Frontend unit tests: `mise run test-frontend-unit`
- Frontend E2E tests: see [frontend-e2e.md](frontend-e2e.md)
- Backend E2E tests: see [backend-e2e.md](backend-e2e.md)
- Cargo unit tests: `mise run test-cargo`
```

Keep: mise command table, build/validation, committing, conventions, error handling, scenario ID persistence.

### 4. Actualize `file-structure.md`

Missing entries to add:

**rhd_test/src/** — add missing files:
- `args.rs` — CLI argument definitions for rhd_test
- `control_server.rs` — HTTP control server for test coordination
- `frontend_test.rs` — Frontend E2E test orchestrator
- `sse_test.rs` — SSE streaming test
- `utils.rs` — Test utilities

**frontend/src/tests/** — add:
- `setup.ts` — Test setup
- `testUtils.ts` — E2E test utilities
- `e2e/chat.test.ts`, `e2e/chat-messageflow.test.ts`, `e2e/chat-streaming.test.ts`

**frontend/** — add missing config files:
- `vitest.config.e2e.ts`
- `vitest.config.unit.ts`

### 5. Create debugging guide → `memory/debugging.md`

New file with "debugging-mode" protocol:

```markdown
# Debugging Guide

## Debugging Mode

When tests fail after 2 iterations of fixing, enter debugging mode.

### Rules
1. Stop trying blind fixes
2. Add debug logs around the issue area
3. All debug prints MUST have prefix `DBG:`
4. Run tests, analyze output
5. When issue found and test fixed — search all `DBG:` in project and remove them

### Debug log format
- Rust: `eprintln!("DBG: <context> = {:?}", value);`
- TypeScript: `console.log("DBG: <context>", value);`

### Cleanup
After fix: `rg "DBG:"` to find and remove all debug prints.
```

### 6. Update `memory/AI.md` index

Add new rows to knowledge base table:

| File | When to read |
|------|-------------|
| `frontend-e2e.md` | When working on frontend E2E tests, test utilities, or test infrastructure |
| `backend-e2e.md` | When working on backend E2E tests, rhd_test crate, mock server, or test scenarios |
| `debugging.md` | When any test fails |

## Execution Order

1. Create `memory/frontend-e2e.md`
2. Create `memory/backend-e2e.md`
3. Create `memory/debugging.md`
4. Update `memory/development.md` — remove extracted sections, add references
5. Update `memory/file-structure.md` — add missing files
6. Update `memory/AI.md` — add new entries to index table
