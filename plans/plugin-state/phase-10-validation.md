# Phase 10: Validation & Cleanup

> Parent plan: [`plans/plugin-state-exposure-plan.md`](../plugin-state-exposure-plan.md)

## Overview

Run the full check/test surface after Phases 1–9, fix everything they flag, and
verify the end-to-end behavior manually. This phase adds no features.

**Dependencies:** all previous phases (1–9).

## Backend Checks

```bash
mise run check-cargo          # cargo check across workspace
mise run test-cargo           # cargo build + cargo test
mise run check-large-files    # no file >400 lines (project skill threshold)
```

Expected touched-crate test suites:

| Crate | What must pass |
|---|---|
| `rhd_chat_api` | Phase 1 serde round-trips, `StateFormat` rejection |
| `rhd_db` | Phase 2 versioning/tombstone/cascade/filter tests |
| `rhd_chat_server` | Phase 3 subscription unit tests + `plugin_state_tests.rs` integration; existing `websocket_tests.rs` unaffected |
| `rhd_chat_client` | Phase 4 dispatch tests; existing tests unaffected |
| `rhd_plugin_mcp` | Phase 5 pool/status/registration tests (note `pool_startup_fails_fast_on_bad_cmd` was intentionally replaced) |

## Frontend Checks

```bash
cd frontend
nvm use && npm test                 # vitest run — all suites
nvm use && npm run type-check       # svelte-check
nvm use && ./node_modules/.bin/eslint --fix src/lib/api/schemas.ts src/lib/api/chatApiImpl.ts src/lib/api/ChatApi.ts src/stores/PluginsStore.ts src/stores/PluginsStore.test.ts src/lib/components/PluginList.svelte src/lib/components/PluginList.test.ts src/lib/components/McpStatusList.svelte src/lib/components/McpStatusList.test.ts src/lib/components/TabView.svelte src/App.svelte
```

## Manual End-to-End Smoke

1. `cargo run -p rhd_chat_server` (default config, fresh DB).
2. Start `rhd_plugin_mcp` with a config containing one good server (the built
   `mcp_stub_server` bin) and one with `cmd: /nonexistent`.
3. `cd frontend && nvm use && npm run dev` → open the app:
   - **Plugins tab**: `mcp` entry shows a collapsed `State (1)` section; expand →
     json `<pre>` with the payload, `json`/`mcpStatus:1` badges, `v1`.
   - **MCPs tab** visible; stub server `OK`, broken server `Error` with the
     spawn message.
4. Trigger a tool-call failure against the stub (`stub:fail` via a chat, or
   kill the stub process) → MCPs tab flips that row to Error **without reload**
   (event-driven; version increments — check the badge `v2`).
5. Kill the plugin process → Plugins tab entry goes Inactive, state section
   remains with "last known"; MCPs tab still visible (state persists).
6. Restart plugin → new push (v3+), "last known" hint disappears; UI never
   shows a lower version than before (monotonicity).
7. `rhd` CLI `plugins` command (if it lists plugins) must still work —
   `getPlugins` untouched.

## Regression Watch

- Existing MCP plugin e2e flows (registration, tool calls) must be unaffected
  by the startup change except for the intentional best-effort behavior.
- `websocket_tests.rs` green ⇒ no accidental change to existing handlers.
- `ChatsListStore`/`ChatStore` untouched ⇒ chat UI suites green.

## Fix Policy

Any failure → fix in the owning phase's scope (don't paper over in a later
phase). If a check reveals a contract mismatch between phases (e.g. field name
drift), the Phase 1 wire contract table is the source of truth.

## Dependencies

- Depends on: Phases 1–9.
- Blocks: Phase 11 (document only what is verified).
