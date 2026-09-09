# Phase 11: Knowledge Base Updates

> Parent plan: [`plans/plugin-state-exposure-plan.md`](../plugin-state-exposure-plan.md)

## Overview

Record the plugin-state feature in the memory base following the established
two-layer pattern (product view in `features/`, implementation in top-level
files). Only what Phase 10 verified gets documented.

**Dependencies:** Phase 10 complete.

## Files to Modify

### 1. `memory/features/plugins.md` (modify)

Add a **"Plugin State"** section (product view — no schemas/paths) after
"Custom Event Coordination":

- What it is: named states per plugin (`key` namespaced by plugin), with
  `format` (`markdown`/`json`) and a `schema` string declaring a well-known
  content format + version (e.g. `mcpStatus:1`, `errors:1`).
- Lifecycle: plugins upsert/remove states; states persist across plugin
  disconnects (shown as "last known" for inactive plugins) and are deleted with
  the plugin (`removePlugin`).
- Versioning contract (consumer-facing): server-assigned, starts at 1, bumps on
  every update **and** removal, monotonic across re-create; consumers must
  ignore events not newer than what they hold.
- Race-free consumption recipe: `getPluginStates` → `subscribePluginStates`
  with held versions (or `0` for latest) → apply response → handle events
  version-gated.
- UI behavior: Plugins tab collapsed state sections; MCPs tab visibility driven
  by any `mcpStatus:1` state.

Update the "MCP Plugin" section's **Behavior** bullet: per-server status now
reported via `mcpStatus:1` state; startup is best-effort (cross-ref
`features/mcp-plugin.md`).

### 2. `memory/features/mcp-plugin.md` (modify)

Product view updates:

- One broken server no longer kills the plugin; it appears as `error` in the
  MCPs tab with its message; healthy servers keep working.
- Status transitions: spawn/list-tools failure and tool-call transport failures
  mark `error`; a later successful call recovers to `ok`. Tool-level `isError`
  results are **not** server failures.
- Config parse errors remain fatal.

### 3. `memory/architecture.md` (modify)

Implementation layer:

- `rhd_chat_api`: `PluginState`/`StateFormat`/`StateVersionRef` types; 5 methods;
  2 events (`pluginStateChanged`, `pluginStateRemoved`).
- `rhd_chat_server`: `handlers/plugin_state.rs`, `plugin_states_subscribers` in
  `SubscriptionManager`; register-before-snapshot subscribe atomicity.
- `rhd_db`: `plugin_states` table (PK `(plugin_id, key)`, `version`,
  `is_removed` tombstone, FK cascade) + 4 access functions.
- `rhd_chat_client`: typed methods + `on_plugin_state_event`.

### 4. `memory/file-structure.md` (modify)

Add rows for the new files: `plugin_states.rs` (db), `handlers/plugin_state.rs`,
`plugins/rhd_plugin_mcp/src/status.rs`, `frontend/src/lib/components/McpStatusList.svelte`,
plus the new method/event files under `rhd_chat_api`.

### 5. `memory/development.md` (modify — Plugin Development patterns)

Add one pattern entry:

### Pattern: Consume Plugin State With Version Gating

**Context:** subscribing to any well-known state schema from a plugin or the
frontend.
**Rule:** get → subscribe with held versions (0 = latest) → apply only strictly
newer versions; ignore older/equal events. Removals are gated the same way.
**Why:** the server broadcasts all state changes unfiltered; catch-up responses
and live events can overlap. Version gating makes duplicates and out-of-order
delivery harmless.

### 6. `memory/MEMORY.md` (modify)

- Index table: extend the `features/plugins.md` "When to read" cell with
  "plugin states, state schemas (mcpStatus:1)".
- Extend the `features/mcp-plugin.md` cell with "per-server status reporting".

### 7. `plugins/README.md` (modify)

In the plugin-development conventions: document `updatePluginState` /
`subscribePluginStates` as the cross-plugin data-exposure mechanism and the
`schema` naming convention `<format>:<version>`.

## Validation

- Cross-check every documented name against the merged code (type names, event
  names, method names, file paths).
- Keep each memory file coherent with its layer: no schemas/paths in
  `features/*`, no product prose in top-level files.

## Dependencies

- Depends on: Phase 10.
- Blocks: nothing (final phase).
