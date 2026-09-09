# Phase 4: rhd_chat_client — Typed State Methods & Event Dispatch

> Parent plan: [`plans/plugin-state-exposure-plan.md`](../plugin-state-exposure-plan.md)

## Overview

Give plugins (Rust clients) typed access to the state protocol: 5 request
methods on `ChatClient`, plus client-side subscription plumbing so
`pluginStateChanged` / `pluginStateRemoved` events reach callbacks — following
the exact `on_custom_event` / `PluginsListSubscription` pattern.

**Scope in:** `client.rs` methods + dispatch arms, `event_stream.rs`
subscription types, unit test for dispatch.
**Scope out:** server behavior (Phase 3), plugin usage (Phase 5).

**Dependencies:** Phase 1 (types + event names). Must land before Phase 5 and
lets Phase 3's integration tests use typed calls.

## Files to Modify

### 1. `packages/rhd_chat_client/src/event_stream.rs` (modify)

Add alongside the other event enums (after `PluginsListEvent`):

```rust
/// Events on plugin states (broadcast to all state subscribers; consumers
/// filter by pluginId/schema/version client-side).
#[derive(Debug, Clone)]
pub enum PluginStateEvent {
    /// A state was created or updated (full stored state incl. new version).
    Changed(PluginStateChangedData),
    /// A state was removed (tombstoned); version is the bumped version.
    Removed(PluginStateRemovedData),
}
```

Imports: extend the `use rhd_chat_api::{...}` list with
`PluginStateChangedData, PluginStateRemovedData`.

Callback alias (after `PluginsListEventCallback`):

```rust
/// Type alias for async plugin state event callbacks.
pub type PluginStateEventCallback =
    Arc<dyn Fn(PluginStateEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;
```

Subscription struct (after `PluginsListSubscription`):

```rust
/// A subscription to plugin state events.
pub(crate) struct PluginStateSubscription {
    pub callback: PluginStateEventCallback,
    pub cancel_rx: oneshot::Receiver<()>,
}
```

`EventSubscriptions`: add field `pub plugin_state_subscriptions: Vec<PluginStateSubscription>,`
+ init in `new()` + retain line in `cleanup()`.

### 2. `packages/rhd_chat_client/src/client.rs` (modify)

**Imports:** extend the `rhd_chat_api::{...}` use list with
`GetPluginStatesParams, GetPluginStatesResult, RemovePluginStateParams,
RemovePluginStateResult, SubscribePluginStatesParams,
SubscribePluginStatesResult, UnsubscribePluginStatesParams,
UnsubscribePluginStatesResult, UpdatePluginStateParams, UpdatePluginStateResult`
(`StateVersionRef` is nested inside the subscribe params — do not import it
directly, it would be an unused-import warning) and from `event_stream` import
`PluginStateEvent, PluginStateEventCallback, PluginStateSubscription`.

**Typed methods** — new section after the "Plugin Methods" block:

```rust
    // ========================================================================
    // Plugin State Methods
    // ========================================================================

    /// Upsert a state owned by this connection's registered plugin.
    /// The result carries the stored state including the server-assigned version.
    pub async fn update_plugin_state(&self, params: UpdatePluginStateParams) -> Result<UpdatePluginStateResult, ClientError> {
        self.send_request("updatePluginState", params).await
    }

    /// Remove (tombstone) one of this plugin's states by key.
    pub async fn remove_plugin_state(&self, params: RemovePluginStateParams) -> Result<RemovePluginStateResult, ClientError> {
        self.send_request("removePluginState", params).await
    }

    /// Query live states, optionally filtered by plugin id and/or schema.
    pub async fn get_plugin_states(&self, params: GetPluginStatesParams) -> Result<GetPluginStatesResult, ClientError> {
        self.send_request("getPluginStates", params).await
    }

    /// Subscribe to plugin state events and atomically catch up on states
    /// newer than the passed versions (pass version 0 to receive the latest).
    pub async fn subscribe_plugin_states(&self, params: SubscribePluginStatesParams) -> Result<SubscribePluginStatesResult, ClientError> {
        self.send_request("subscribePluginStates", params).await
    }

    /// Unsubscribe from plugin state events.
    pub async fn unsubscribe_plugin_states(&self, params: UnsubscribePluginStatesParams) -> Result<UnsubscribePluginStatesResult, ClientError> {
        self.send_request("unsubscribePluginStates", params).await
    }
```

**Dispatch arms** — in `dispatch_event`, after the `"pluginRemoved"` arm and
before `"customEvent"`:

```rust
            "pluginStateChanged" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::PluginStateChangedData>(event.data.clone()) {
                    for sub in &subs.plugin_state_subscriptions {
                        let callback = sub.callback.clone();
                        let evt = PluginStateEvent::Changed(data.clone());
                        tokio::spawn(async move {
                            (callback)(evt).await;
                        });
                    }
                }
            }
            "pluginStateRemoved" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::PluginStateRemovedData>(event.data.clone()) {
                    for sub in &subs.plugin_state_subscriptions {
                        let callback = sub.callback.clone();
                        let evt = PluginStateEvent::Removed(data.clone());
                        tokio::spawn(async move {
                            (callback)(evt).await;
                        });
                    }
                }
            }
```

**Subscription helper** — after `on_plugins_list_event` (same sync-fn shape,
`tokio::spawn` to push under the lock):

```rust
    /// Subscribe to plugin state events (pluginStateChanged, pluginStateRemoved).
    ///
    /// All state changes are broadcast to every subscriber; filter by
    /// `state.pluginId` / `state.schema` / version inside the callback.
    /// Returns a cancellation token — dropping or cancelling it unsubscribes.
    pub fn on_plugin_state_event<F, Fut>(&self, callback: F) -> CancellationToken
    where
        F: Fn(PluginStateEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (token, cancel_rx) = CancellationToken::new();
        let boxed_callback: PluginStateEventCallback =
            Arc::new(move |event| Box::pin(callback(event)));

        let subscription = PluginStateSubscription {
            callback: boxed_callback,
            cancel_rx,
        };

        let subscriptions = Arc::clone(&self.subscriptions);
        tokio::spawn(async move {
            let mut subs = subscriptions.lock().await;
            subs.plugin_state_subscriptions.push(subscription);
        });

        token
    }
```

**`lib.rs`:** ensure `PluginStateEvent` is re-exported if the crate re-exports
`ChatEvent`/`PluginsListEvent` (check the existing `pub use event_stream::{...}`
list in `packages/rhd_chat_client/src/lib.rs` and add the new enum + callback
type the same way).

## Tests

Add to the existing `#[cfg(test)] mod tests` in `client.rs` (create one if
absent — check first with: `grep -n "mod tests" packages/rhd_chat_client/src/client.rs`):

```rust
    #[tokio::test]
    async fn dispatch_plugin_state_changed_reaches_callback() {
        let subscriptions = Arc::new(Mutex::new(EventSubscriptions::new()));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let token = {
            // register a callback that forwards events into rx
            // (construct PluginStateEventCallback manually, push into
            //  subs.plugin_state_subscriptions — same fields as the struct)
        };
        let event = Event::new(
            "pluginStateChanged",
            serde_json::json!({ "state": {
                "pluginId": "p1", "key": "status", "content": "{}",
                "format": "json", "schema": "mcpStatus:1", "version": 1,
                "updatedAt": "2026-09-05 22:41:07"
            }}),
        );
        ChatClient::dispatch_event(&event, &subscriptions).await;
        let received = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await.unwrap();
        assert!(matches!(received, Some(PluginStateEvent::Changed(d)) if d.state.version == 1));
        token.cancel();
    }
```

Plus an analogous `pluginStateRemoved` test. Run:

```bash
cargo test -p rhd_chat_client
mise run check-cargo
```

## Implementation Notes

1. **Callbacks are spawned, never awaited inline** — the read task must keep
   draining frames (memory/development.md: inline-awaited callbacks that issue
   their own request deadlock the client). The arms above follow this exactly.
2. **`on_plugin_state_event` is sync + spawn-push** like `on_plugins_list_event`;
   `CancellationToken::drop` cancels — callers must keep the token alive
   (see `plugin.rs` step 8 comment in the MCP plugin).
3. **No server-side filtering** — one subscription list, all events, consumers
   filter. Version-gating is a consumer responsibility (documented in Phase 7).
4. **`dispatch_event` is private** — the unit test lives in `client.rs`'s own
   test module to reach it.

## Dependencies

- Depends on: Phase 1.
- Blocks: Phase 5 (plugin pushes state + may watch others' state), Phase 3
  typed integration tests.
