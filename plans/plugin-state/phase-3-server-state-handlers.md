# Phase 3: rhd_chat_server — State Handlers & Plugin-States Broadcast

> Parent plan: [`plans/plugin-state-exposure-plan.md`](../plugin-state-exposure-plan.md)

## Overview

Expose the 5 state methods over WebSocket: route them in `handlers/mod.rs`,
implement them in a new `handlers/plugin_state.rs`, add a plugin-states
subscriber set to `SubscriptionManager`, and broadcast
`pluginStateChanged` / `pluginStateRemoved` events. Map DB rows
(`PluginStateRow`) to API `PluginState`.

**Scope in:** subscriptions, event helpers, handlers, routing, integration tests.
**Scope out:** client typed methods (Phase 4), plugin-side usage (Phase 5).

**Dependencies:** Phase 1 (API types) and Phase 2 (DB methods) must be done.

## Files to Modify/Create

### 1. `packages/rhd_chat_server/src/subscriptions.rs` (modify)

Add a field to `SubscriptionManager`:

```rust
    /// Set of connection IDs subscribed to plugin state change events.
    plugin_states_subscribers: HashSet<ConnectionId>,
```

Cleanup in `unregister_connection` (next to `plugins_list_subscribers.remove`):

```rust
        self.plugin_states_subscribers.remove(connection_id);
```

Add methods after `broadcast_to_plugins_list` (identical shape):

```rust
    /// Subscribe a connection to plugin state change events.
    pub fn subscribe_plugin_states(&mut self, connection_id: &str) {
        self.plugin_states_subscribers.insert(connection_id.to_string());
    }

    /// Unsubscribe a connection from plugin state change events.
    pub fn unsubscribe_plugin_states(&mut self, connection_id: &str) {
        self.plugin_states_subscribers.remove(connection_id);
    }

    /// Broadcast an event to all subscribers of plugin state events.
    pub fn broadcast_to_plugin_states(&self, event: Event) {
        let event_json = match serde_json::to_string(&event) {
            Ok(j) => j,
            Err(_) => return,
        };
        for connection_id in &self.plugin_states_subscribers {
            if let Some(sender) = self.connections.get(connection_id) {
                let _ = sender.send(event_json.clone());
            }
        }
    }
```

Unit tests (mirror `test_subscription_manager_plugins_list_subscription`):
subscribe → broadcast → receiver gets it; unsubscribe → nothing;
`unregister_connection` cleans the set.

### 2. `packages/rhd_chat_server/src/events.rs` (modify)

Append event constructors (same style as `tools_updated_event`):

```rust
/// Create a `pluginStateChanged` event.
pub fn plugin_state_changed_event(state: rhd_chat_api::PluginState) -> Event {
    let data = rhd_chat_api::events::PluginStateChangedData { state };
    Event::new("pluginStateChanged", serde_json::to_value(data).unwrap())
}

/// Create a `pluginStateRemoved` event.
pub fn plugin_state_removed_event(plugin_id: &str, key: &str, version: i64) -> Event {
    let data = rhd_chat_api::events::PluginStateRemovedData {
        plugin_id: plugin_id.to_string(),
        key: key.to_string(),
        version,
    };
    Event::new("pluginStateRemoved", serde_json::to_value(data).unwrap())
}
```

### 3. `packages/rhd_chat_server/src/handlers/plugin_state.rs` (new)

Five handlers. Shared row→API conversion first:

```rust
//! Plugin state handlers.

use serde_json::Value;

use rhd_chat_api::common::{PluginState, StateFormat, StateVersionRef};
use rhd_chat_api::methods::{
    GetPluginStatesParams, GetPluginStatesResult, RemovePluginStateParams, RemovePluginStateResult,
    SubscribePluginStatesParams, SubscribePluginStatesResult, UnsubscribePluginStatesParams,
    UnsubscribePluginStatesResult, UpdatePluginStateParams, UpdatePluginStateResult,
};
use rhd_chat_api::protocol::Response;
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;
use rhd_db::PluginStateRow; // re-exported at crate root (see Phase 2 §3b)

use crate::error::ServerError;
use crate::events::{plugin_state_changed_event, plugin_state_removed_event};
use crate::subscriptions::SharedSubscriptionManager;

/// Map a DB row to the API type. The CHECK constraint guarantees the format
/// string is one of the two known values; anything else is a bug → error.
fn row_to_api_state(row: PluginStateRow) -> Result<PluginState, ServerError> {
    let format = match row.format.as_str() {
        "markdown" => StateFormat::Markdown,
        "json" => StateFormat::Json,
        other => {
            return Err(ServerError::Internal(format!("invalid stored state format: {other}")))
        }
    };
    Ok(PluginState {
        plugin_id: row.plugin_id,
        key: row.key,
        content: row.content,
        format,
        schema: row.schema,
        version: row.version,
        updated_at: row.updated_at,
    })
}
```

**`update_plugin_state`** — signature mirrors `tools::add_tools`
(`plugin_id: Option<&str>` resolved by the caller):

```rust
pub async fn update_plugin_state(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    plugin_id: Option<&str>,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let plugin_id = match plugin_id {
        Some(id) => id,
        None => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                "Plugin must be registered before updating state",
            ))?);
        }
    };
    let params: UpdatePluginStateParams = /* from_value, invalid_request on err */;

    let format_str = match params.format {
        StateFormat::Markdown => "markdown",
        StateFormat::Json => "json",
    };
    let row = db.upsert_plugin_state(
        plugin_id, &params.key, &params.content, format_str, &params.schema,
    )?;
    let state = row_to_api_state(row)?;

    let event = plugin_state_changed_event(state.clone());
    let manager = subscription_manager.read().await;
    manager.broadcast_to_plugin_states(event);

    let result = UpdatePluginStateResult { state };
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}
```

**`remove_plugin_state`** — same plugin guard; then:

```rust
    let params: RemovePluginStateParams = /* ... */;
    if let Some(version) = db.remove_plugin_state(plugin_id, &params.key)? {
        let event = plugin_state_removed_event(plugin_id, &params.key, version);
        let manager = subscription_manager.read().await;
        manager.broadcast_to_plugin_states(event);
    }
    // Removing a non-existent/already-removed state is a silent success.
    let result = RemovePluginStateResult {};
    /* Response::success */
```

**`get_plugin_states`** — no plugin guard (any connection may read):

```rust
pub async fn get_plugin_states(
    params: Value, db: &ChatDb, request_id: &str,
) -> Result<Value, ServerError> {
    let params: GetPluginStatesParams = /* ... */;
    let rows = db.get_plugin_states(params.plugin_id.as_deref(), params.schema.as_deref())?;
    let states = rows.into_iter().map(row_to_api_state).collect::<Result<Vec<_>, _>>()?;
    let result = GetPluginStatesResult { states };
    /* Response::success */
}
```

**`subscribe_plugin_states`** — the race-free core. Register under the write
lock **first**, release, **then** read the catch-up snapshot. Any update that
lands in between is either already in the snapshot or delivered as a live event;
duplicates are made harmless by client-side version gating.

```rust
pub async fn subscribe_plugin_states(
    params: Value,
    db: &ChatDb,
    request_id: &str,
    connection_id: &str,
    subscription_manager: &SharedSubscriptionManager,
) -> Result<Value, ServerError> {
    let params: SubscribePluginStatesParams = /* ... */;

    // 1. Register as subscriber (write lock held only for the set insert).
    {
        let mut manager = subscription_manager.write().await;
        manager.subscribe_plugin_states(connection_id);
    }

    // 2. Catch-up: return current state for refs whose stored version is newer
    //    than requested (version 0 => always newer). Tombstones are skipped —
    //    nothing to deliver for a removed state.
    let mut states = Vec::new();
    for StateVersionRef { plugin_id, key, version } in &params.states {
        if let Some(row) = db.get_plugin_state(plugin_id, key)? {
            if !row.is_removed && row.version > *version {
                states.push(row_to_api_state(row)?);
            }
        }
    }

    let result = SubscribePluginStatesResult { states };
    /* Response::success */
}
```

**`unsubscribe_plugin_states`** — trivial, mirrors
`plugin::unsubscribe_plugins_list` (write lock, `unsubscribe_plugin_states`,
empty result).

### 4. `packages/rhd_chat_server/src/handlers/mod.rs` (modify)

- `pub mod plugin_state;` after `pub mod plugin;`.
- Route the 5 methods after the existing plugin-method block. update/remove
  resolve the owning plugin exactly like `addTools`:

```rust
        // Plugin state methods
        "updatePluginState" => {
            let plugin_id = {
                let registry = plugin_registry.read().await;
                let plugins = registry.get_plugins_for_connection(connection_id);
                plugins.into_iter().next()
            };
            plugin_state::update_plugin_state(request.params, db, &request_id, plugin_id.as_deref(), &subscription_manager).await
        }
        "removePluginState" => {
            let plugin_id = { /* same resolution */ };
            plugin_state::remove_plugin_state(request.params, db, &request_id, plugin_id.as_deref(), &subscription_manager).await
        }
        "getPluginStates" => plugin_state::get_plugin_states(request.params, db, &request_id).await,
        "subscribePluginStates" => plugin_state::subscribe_plugin_states(request.params, db, &request_id, connection_id, subscription_manager).await,
        "unsubscribePluginStates" => plugin_state::unsubscribe_plugin_states(request.params, db, &request_id, connection_id, subscription_manager).await,
```

Note: `subscription_manager` is moved into the last arm of the existing match
style — follow how `subscribePluginsList` is wired today (clone where needed).

### 5. `packages/rhd_chat_server/tests/plugin_state_tests.rs` (new)

Separate file (keep `websocket_tests.rs` from growing past 1443 lines). Copy the
`start_test_server` / `connect_client` harness verbatim from
`websocket_tests.rs:31-94` (they are private to that file).

Tests (all via `rhd_chat_client::ChatClient` — uses Phase 4 methods if this file
is written after Phase 4; otherwise raw `Request`/`Response` JSON via
`tokio_tungstenite`. **Order: write Phase 4 first if you prefer typed tests.**):

1. `update_requires_registered_plugin` — fresh connection → `updatePluginState`
   → error response (`ClientError::Server` with code `invalid_request`).
2. `upsert_versioning_flow` — register plugin "ps-test"; update → result.state
   `.version == 1`; update again → `.version == 2`; `getPluginStates` returns the
   v2 state with matching `content/format/schema`.
3. `remove_bumps_version_and_hides` — after remove: `getPluginStates` empty;
   subscriber received `pluginStateRemoved` with `version == 3`.
4. `recreate_after_remove_is_monotonic` — v1 → remove v2 → update v3.
5. `subscribe_receives_broadcasts` — conn B `subscribePluginStates({states: []})`;
   conn A (plugin) updates → B's event stream yields `pluginStateChanged` with
   the full state.
6. `subscribe_catchup_returns_newer_only` — plugin updates to v2; B subscribes
   with `[{pluginId, key, version: 1}]` → response contains the v2 state;
   subscribing with `version: 2` → empty; `version: 0` → latest.
7. `get_plugin_states_filters` — two plugins with keys/schemas; filter by
   `pluginId`, by `schema`, and unfiltered.
8. `remove_plugin_cascades_states` — register, update, `removePlugin` →
   `getPluginStates` empty.
9. `states_survive_disconnect` — plugin conn dropped (close WS) → `getPluginStates`
   still returns the state (deactivate keeps rows).

For event assertions use `tokio::time::timeout` + the client's event
subscriptions (Phase 4's `on_plugin_state_event`), or raw-frame parsing.

## Tests

```bash
cargo test -p rhd_chat_server plugin_state
mise run check-cargo
```

## Implementation Notes

1. **Register-before-snapshot** is the invariant that makes get→subscribe
   race-free; do not reorder steps 1/2 in `subscribe_plugin_states`, and do not
   hold the write lock across the DB reads (would stall all broadcasts).
2. **No chat-version bump** for state changes — states are global, not chat
   state; only `plugin_states_subscribers` receive events.
3. **`pluginStateRemoved` carries the version** so version-gated consumers can
   ignore a late removal that races an older update.
4. **Update result returns the stored state** — the writer learns its assigned
   version without a follow-up `getPluginStates` (used by the MCP plugin dedup).
5. **Error surface**: reuse `ErrorResponse::invalid_request` for unregistered
   plugin and bad params; `ServerError::Internal` only for the impossible
   stored-format case (CHECK constraint makes it unreachable in practice).
6. Keep the new handler file under 500 lines; if it grows, split subscribe/get
   into `plugin_state_read.rs` (not expected).

## Dependencies

- Depends on: Phase 1 (`rhd_chat_api` types), Phase 2 (`ChatDb` methods).
  Test file additionally benefits from Phase 4 typed client methods.
- Blocks: Phase 4 (event names must exist to dispatch), Phase 5 (plugin calls
  these methods), Phases 6–9 (frontend consumes events).
