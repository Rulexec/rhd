# Phase 1: rhd_chat_api — Plugin State Types, Methods, Events

> Parent plan: [`plans/plugin-state-exposure-plan.md`](../plugin-state-exposure-plan.md)

## Overview

Define every wire type for the plugin-state feature in the `rhd_chat_api` crate:
the shared `PluginState`/`StateFormat`/`StateVersionRef` types, Params/Result structs
for the 5 new protocol methods, and Data structs for the 2 new events. This phase is
pure data modeling — no server/client logic. Every later phase imports these types,
so names/signatures here are the cross-phase contract.

**Scope in:** types + serde tests.
**Scope out:** DB, handlers, client methods, frontend.

**Dependencies:** none. Must complete before Phases 2–7.

## Wire Contract (fixed — other phases rely on this)

| Method | Params JSON | Result JSON |
|---|---|---|
| `updatePluginState` | `{key, content, format, schema}` | `{state: PluginState}` |
| `removePluginState` | `{key}` | `{}` |
| `getPluginStates` | `{pluginId?, schema?}` | `{states: [PluginState]}` |
| `subscribePluginStates` | `{states: [{pluginId, key, version}]}` | `{states: [PluginState]}` |
| `unsubscribePluginStates` | `{}` | `{}` |

| Event | Data JSON |
|---|---|
| `pluginStateChanged` | `{state: PluginState}` |
| `pluginStateRemoved` | `{pluginId, key, version}` |

`PluginState` JSON shape:
```json
{
  "pluginId": "mcp",
  "key": "status",
  "content": "{\"mcp\":[...]}",
  "format": "json",
  "schema": "mcpStatus:1",
  "version": 3,
  "updatedAt": "2026-09-05 22:41:07"
}
```

- `format`: `"markdown"` | `"json"` (lowercase on the wire).
- `version`: server-assigned, starts at `1`, increments on every update **and** on remove (tombstone), monotonic across re-creation. Clients never write it.
- `updatedAt`: opaque SQLite `datetime('now')` UTC string (`"YYYY-MM-DD HH:MM:SS"`). Deliberately **not** `DateTime<Utc>`: the DB stores exactly this string, and passing it through unchanged avoids a lossy parse/serialize round-trip. Frontend treats it as an opaque display string.

## Files to Modify/Create

### 1. `packages/rhd_chat_api/src/common.rs` (modify)

Append after `PluginSummary` (before `PendingEvent`):

```rust
/// Format of a plugin state's `content`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StateFormat {
    Markdown,
    Json,
}

/// A single named state exposed by a plugin.
///
/// Identified by `(plugin_id, key)`; `version` is server-managed and strictly
/// monotonic per pair across updates and removes (tombstones).
///
/// # Example JSON
/// ```json
/// {
///   "pluginId": "mcp",
///   "key": "status",
///   "content": "{\"mcp\":[]}",
///   "format": "json",
///   "schema": "mcpStatus:1",
///   "version": 1,
///   "updatedAt": "2026-09-05 22:41:07"
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginState {
    pub plugin_id: String,
    pub key: String,
    pub content: String,
    pub format: StateFormat,
    pub schema: String,
    pub version: i64,
    pub updated_at: String,
}

/// One `(pluginId, key, version)` entry of `subscribePluginStates` params:
/// the version the client already holds. The server returns the current state
/// for every ref whose stored version is greater (catch-up), or nothing newer.
/// `version: 0` means "I hold nothing — send the latest".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StateVersionRef {
    pub plugin_id: String,
    pub key: String,
    pub version: i64,
}
```

Add serialization tests in the existing `mod tests`: `PluginState` round-trip
asserting `"format":"json"`, `"pluginId"`, `"updatedAt"`; `StateFormat` rejects
unknown values (`serde_json::from_str::<StateFormat>("\"yaml\"")` is `Err`).

### 2. `packages/rhd_chat_api/src/methods/update_plugin_state.rs` (new)

```rust
//! `updatePluginState` method types.
//!
//! Upsert a state owned by the calling plugin. The plugin id is derived from
//! the connection's registry entry — there is no `pluginId` param.

use serde::{Deserialize, Serialize};

use crate::common::{PluginState, StateFormat};

/// Parameters for the `updatePluginState` method.
///
/// # Example JSON
/// ```json
/// { "key": "status", "content": "{\"mcp\":[]}", "format": "json", "schema": "mcpStatus:1" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePluginStateParams {
    pub key: String,
    pub content: String,
    pub format: StateFormat,
    pub schema: String,
}

/// Result of the `updatePluginState` method — the stored state including the
/// server-assigned `version` so the writer learns its new version immediately.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePluginStateResult {
    pub state: PluginState,
}
```
(+ round-trip test, mirroring `add_tools.rs`.)

### 3. `packages/rhd_chat_api/src/methods/remove_plugin_state.rs` (new)

`RemovePluginStateParams { pub key: String }`, `RemovePluginStateResult {}` — same
file skeleton/tests as above.

### 4. `packages/rhd_chat_api/src/methods/get_plugin_states.rs` (new)

```rust
use serde::{Deserialize, Serialize};

use crate::common::PluginState;

/// Parameters for the `getPluginStates` method. Both filters optional;
/// `{}` returns all live (non-removed) states of all plugins.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GetPluginStatesParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetPluginStatesResult {
    pub states: Vec<PluginState>,
}
```
Test: `serde_json::from_value::<GetPluginStatesParams>(json!({}))` succeeds (empty
params must be accepted — `#[serde(default)]` on the struct).

### 5. `packages/rhd_chat_api/src/methods/subscribe_plugin_states.rs` (new)

```rust
use serde::{Deserialize, Serialize};

use crate::common::{PluginState, StateVersionRef};

/// Parameters for the `subscribePluginStates` method.
///
/// `states` is the catch-up list: for each ref the server returns the current
/// state if its version is greater than the passed one. An empty list (or
/// omitted — `#[serde(default)]`) subscribes without catch-up.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SubscribePluginStatesParams {
    pub states: Vec<StateVersionRef>,
}

/// Result: current states newer than requested (possibly empty).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubscribePluginStatesResult {
    pub states: Vec<PluginState>,
}
```

### 6. `packages/rhd_chat_api/src/methods/unsubscribe_plugin_states.rs` (new)

Empty `UnsubscribePluginStatesParams {}` / `UnsubscribePluginStatesResult {}`,
mirroring `unsubscribe_plugins_list.rs` exactly.

### 7. `packages/rhd_chat_api/src/methods/mod.rs` (modify)

The existing file is alphabetical. Note `_` (0x5F) sorts before `s`, so
`get_plugin_states` precedes `get_plugins` and `subscribe_plugin_states`
precedes `subscribe_plugins_list`. Exact insertion points:

```rust
pub mod get_plugin_states;          // after get_pending_acks, before get_plugins
pub mod remove_plugin_state;        // after remove_plugin, before remove_tools
pub mod subscribe_plugin_states;    // after subscribe_chats_list, before subscribe_plugins_list
pub mod unsubscribe_plugin_states;  // after unsubscribe_chats_list, before unsubscribe_plugins_list
pub mod update_plugin_state;        // after update_message, before update_queue_message
```

and the five matching `pub use ...::*;` lines in the same relative order.

### 8. `packages/rhd_chat_api/src/events/plugin_state_changed.rs` (new)

```rust
//! `pluginStateChanged` event data.
//!
//! Emitted to plugin-state subscribers when a state is created or updated
//! (upsert). Carries the full stored state including the new version.

use serde::{Deserialize, Serialize};

use crate::common::PluginState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginStateChangedData {
    pub state: PluginState,
}
```
(+ round-trip test.)

### 9. `packages/rhd_chat_api/src/events/plugin_state_removed.rs` (new)

```rust
//! `pluginStateRemoved` event data.
//!
//! Emitted when a state is removed (tombstoned). `version` is the bumped
//! version so consumers can version-gate stale events.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginStateRemovedData {
    pub plugin_id: String,
    pub key: String,
    pub version: i64,
}
```

### 10. `packages/rhd_chat_api/src/events/mod.rs` (modify)

Add `pub mod plugin_state_changed; pub mod plugin_state_removed;` and
`pub use` lines (alphabetical: after `plugin_removed`).

### 11. `packages/rhd_chat_api/src/lib.rs` (modify)

- `pub use common::{...}` line: add `PluginState, StateFormat, StateVersionRef`.
- `pub use events::{...}`: add `PluginStateChangedData, PluginStateRemovedData`.
- `pub use methods::{...}`: add all 10 new Params/Result names.
- Update the crate-doc "Shared types" bullet to mention `PluginState`.

## Tests

All in-crate serde round-trip tests as sketched above. Run:

```bash
cargo test -p rhd_chat_api
mise run check-cargo
```

## Implementation Notes

1. **`updated_at: String`** — see Wire Contract rationale; do not "fix" this to
   `DateTime<Utc>`, the DB column is `datetime('now')` and a chrono round-trip
   would need parsing with `%Y-%m-%d %H:%M:%S`.
2. **`StateFormat` is `Copy`** — handlers pass it by value into DB calls.
3. **`#[serde(default)]` on params structs** — the frontend sends `{}` for
   `getPluginStates`/`subscribePluginStates`; deserialization must not fail.
4. **No `pluginId` in update/remove params** — ownership comes from the
   connection's registered plugin (anti-spoofing, mirrors `addTools`).

## Dependencies

- Depends on: nothing.
- Blocks: Phases 2 (row→API mapping), 3 (handlers), 4 (client), 5 (plugin),
  6–9 (frontend mirrors these shapes in zod).
