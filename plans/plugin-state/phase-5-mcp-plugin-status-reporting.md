# Phase 5: rhd_plugin_mcp — Best-Effort Startup & `mcpStatus:1` State Reporting

> Parent plan: [`plans/plugin-state-exposure-plan.md`](../plugin-state-exposure-plan.md)

## Overview

Make the MCP plugin expose its server fleet as plugin state:
`key: "status"`, `format: "json"`, `schema: "mcpStatus:1"`, content
`{"mcp":[{"id","name","status":"ok"|"error","error"?}]}`.

Two coupled changes:

1. **Best-effort pool startup** (replaces fail-fast, AD-4): a server that fails
   to spawn or fails `list_tools` no longer aborts the plugin — it is recorded
   as `"error"` in the state payload while healthy servers keep working.
2. **Status tracking + push**: a tracker holds per-server status; the plugin
   pushes the serialized payload via `updatePluginState` at startup and whenever
   a status changes (startup failure, tool-call transport failure, recovery on
   next success). No polling.

**Scope in:** `mcp_pool.rs`, new `status.rs`, `plugin.rs`, `tool_handler.rs`,
`lib.rs`, tests, plugin `README.md`.
**Scope out:** server/client/frontend (Phases 1–4, 6–9).

**Dependencies:** Phases 1 + 4 (client typed methods). Phase 3 needed only for
the integration test that reads state back from a live server.

## Files to Modify/Create

### 1. `plugins/rhd_plugin_mcp/src/status.rs` (new)

```rust
//! Per-MCP-server run status tracking and `mcpStatus:1` state reporting.

use std::collections::HashMap;

use rhd_chat_api::{StateFormat, UpdatePluginStateParams};
use rhd_chat_client::ChatClient;
use serde::Serialize;
use tokio::sync::Mutex;

use crate::mcp_pool::ServerStartupReport;

/// One entry of the `mcpStatus:1` payload.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpStatusEntry {
    pub id: String,
    pub name: String,
    /// "ok" | "error"
    pub status: &'static str,
    /// Present only for errored servers: startup / broken-protocol /
    /// crash-on-call message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct McpStatusPayload {
    pub mcp: Vec<McpStatusEntry>,
}

/// The state key and schema this plugin publishes.
pub const STATE_KEY: &str = "status";
pub const STATE_SCHEMA: &str = "mcpStatus:1";

/// Live status of every configured server, in config order (stable payload).
pub struct McpStatusTracker {
    order: Vec<(String, String)>, // (id, name) — fixed at construction
    statuses: Mutex<HashMap<String, ServerRunStatus>>,
    last_pushed: Mutex<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerRunStatus {
    pub ok: bool,
    pub error: Option<String>,
}

impl McpStatusTracker {
    /// Build from the resolved config order; all servers start "unknown-ok"
    /// and are immediately corrected by `record_startup`.
    pub fn new(order: Vec<(String, String)>) -> Self { /* ... */ }

    /// Apply per-server startup outcomes from the pool.
    pub async fn record_startup(&self, reports: &[ServerStartupReport]) { /* ... */ }

    pub async fn mark_error(&self, server_id: &str, message: String) { /* ... */ }
    pub async fn mark_ok(&self, server_id: &str) { /* ... */ }

    /// Serialize the payload in config order.
    pub async fn payload_json(&self) -> String {
        let statuses = self.statuses.lock().await;
        let entries: Vec<McpStatusEntry> = self
            .order
            .iter()
            .map(|(id, name)| match statuses.get(id) {
                Some(s) if s.ok => McpStatusEntry {
                    id: id.clone(), name: name.clone(), status: "ok", error: None,
                },
                Some(s) => McpStatusEntry {
                    id: id.clone(), name: name.clone(),
                    status: "error", error: s.error.clone(),
                },
                None => McpStatusEntry {
                    id: id.clone(), name: name.clone(),
                    status: "error",
                    error: Some("status not recorded".to_string()),
                },
            })
            .collect();
        serde_json::to_string(&McpStatusPayload { mcp: entries }).unwrap()
    }

    /// Push state only when the payload differs from the last successful push
    /// (dedup — the server still assigns versions; this saves traffic).
    pub async fn push_if_changed(&self, client: &ChatClient) -> Result<(), StatusReportError> {
        let json = self.payload_json().await;
        if self.last_pushed.lock().await.as_deref() == Some(json.as_str()) {
            return Ok(());
        }
        client
            .update_plugin_state(UpdatePluginStateParams {
                key: STATE_KEY.to_string(),
                content: json.clone(),
                format: StateFormat::Json,
                schema: STATE_SCHEMA.to_string(),
            })
            .await
            .map_err(|e| StatusReportError::Push(e.to_string()))?;
        *self.last_pushed.lock().await = Some(json);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StatusReportError {
    #[error("failed to push mcp status state: {0}")]
    Push(String),
}
```

`ServerStartupReport` is defined in `mcp_pool.rs` (below) and re-used here.
All tracker methods use `tokio::sync::Mutex`. Lock discipline: never hold a
tracker lock across a network await — `push_if_changed` builds the payload
(lock released), then sends, then updates `last_pushed` under a short lock.

### 2. `plugins/rhd_plugin_mcp/src/mcp_pool.rs` (modify)

**Startup becomes best-effort.** Replace the `Result<Self, PoolError>` startup:

```rust
/// Outcome of spawning one configured server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerStartupReport {
    pub id: String,
    pub name: String,
    /// None = healthy; Some(message) = spawn or list_tools failure.
    pub error: Option<String>,
}

impl McpPool {
    /// Spawn and initialize every configured server. A failing server is
    /// reported in `Vec<ServerStartupReport>` and excluded from routing —
    /// the rest of the pool still starts (best-effort, replaces AD-4).
    pub async fn startup(config: &PluginConfig) -> (Self, Vec<ServerStartupReport>) {
        let mut servers = HashMap::new();
        let mut routes = HashMap::new();
        let mut reports = Vec::new();

        for entry in &config.servers {
            let outcome = async {
                let client = McpClient::connect(&entry.cmd, &entry.args, entry.cwd.as_deref(), &entry.env)
                    .await
                    .map_err(|e| format!("spawn/initialize failed: {e}"))?;
                let mcp_tools = client
                    .list_tools()
                    .await
                    .map_err(|e| format!("tools/list failed: {e}"))?;
                Ok::<_, String>((client, mcp_tools))
            }
            .await;

            match outcome {
                Ok((client, mcp_tools)) => {
                    /* existing route/chat_tools building + insert into servers */
                    reports.push(ServerStartupReport { id: entry.id.clone(), name: entry.name.clone(), error: None });
                }
                Err(message) => {
                    tracing::error!(server = %entry.name, id = %entry.id, error = %message, "MCP server failed to start");
                    reports.push(ServerStartupReport { id: entry.id.clone(), name: entry.name.clone(), error: Some(message) });
                }
            }
        }

        (Self { servers, routes }, reports)
    }
}
```

`PoolError::Spawn`/`ListTools` variants become unused → delete them (keep
`UnknownServer` and `Call`). `server_configs()` stays (used by gating). Add
`pub fn server_ids(&self) -> Vec<(String, String)>` returning `(id, name)` in
**config order** for the tracker (iterate `config.servers` order — store the
ordered list on the pool, since `servers` is a HashMap).

### 3. `plugins/rhd_plugin_mcp/src/plugin.rs` (modify)

Reorder + wire the tracker:

```rust
pub async fn run_plugin(server_url, plugin_id, worktree, config) -> Result<(), PluginError> {
    // 1. Connect + register FIRST — state must be pushable even if every
    //    server fails to start.
    let client = Arc::new(ChatClient::connect_with_retry(server_url).await ...);
    client.register_plugin(RegisterPluginParams { plugin_id: ... }).await ...;

    // 2. Best-effort pool startup; build tracker; push initial state.
    let (pool, reports) = McpPool::startup(&config).await;
    let pool = Arc::new(pool);
    let tracker = Arc::new(McpStatusTracker::new(pool.server_ids()));
    tracker.record_startup(&reports).await;
    tracker.push_if_changed(&client).await.map_err(PluginError::StatusReport)?;
    tracing::info!("MCP status state pushed ({} servers)", reports.len());

    // 3..: pending acks, chat monitor, registration, event handlers — unchanged,
    //      except the tool-call closure also clones `tracker`.
    let _tool_call_token = client.on_tool_call(0, pool.all_tool_names(), move |event| {
        let client = Arc::clone(&client_for_tools);
        let pool = Arc::clone(&pool_for_tools);
        let regs = Arc::clone(&regs_for_tools);
        let tracker = Arc::clone(&tracker_for_tools);
        async move {
            tool_handler::handle_tool_calls(client, pool, regs, tracker, event).await;
        }
    });
    ...
}
```

- `PluginError::Pool(String)` variant: delete (startup no longer fails); add
  `#[error("status reporting failed: {0}")] StatusReport(String)`.
- Remove the old step-1 pool block and its `PluginError::Pool` mapping.

### 4. `plugins/rhd_plugin_mcp/src/tool_handler.rs` (modify)

- `handle_tool_calls` / `handle_single_call` take `tracker: Arc<McpStatusTracker>`.
- In step 4 (`pool.call_tool`):
  - `Err(e)` → `tracker.mark_error(&route.server_id, format!("tool call failed: {e}")).await;`
    then `let _ = tracker.push_if_changed(&client).await;` — **then** continue
    with the existing error-as-tool-content path (the AI loop must still be
    answered).
  - `Ok(result)` → if the server was previously marked, `tracker.mark_ok(&route.server_id).await;
    let _ = tracker.push_if_changed(&client).await;` (recovery).
- `result.is_error == Some(true)` is a **tool-level** error, not a server
  failure — do NOT mark status (documented in notes).

### 5. `plugins/rhd_plugin_mcp/src/lib.rs` (modify)

Add `pub mod status;` alongside the other modules.

### 6. Tests

**`tests/pool_test.rs`** — update to the new signature:

```rust
#[tokio::test]
async fn pool_startup_lists_and_routes_stub_tools() {
    let (pool, reports) = McpPool::startup(&config_with("stub", None)).await;
    assert_eq!(reports[0].error, None);
    /* existing assertions unchanged */
}

#[tokio::test]
async fn pool_startup_reports_bad_cmd_without_failing() {
    let mut cfg = config_with("bad", None);
    cfg.servers[0].cmd = "/nonexistent/mcp-binary".to_string();
    let (pool, reports) = McpPool::startup(&cfg).await;
    assert_eq!(reports[0].error.as_deref().unwrap(), "spawn/initialize failed: ..."); // prefix check via .starts_with
    assert!(pool.route("bad:echo").is_none());
    assert!(pool.all_tool_names().is_empty());
}
```

(Replaces `pool_startup_fails_fast_on_bad_cmd` — the fail-fast contract is gone.)

**New `tests/status_test.rs`** (unit-ish, no server needed):

- `payload_shape_matches_schema`: tracker with one ok + one error →
  `payload_json()` parses to `{"mcp":[{"id","name","status":"ok"},
  {"id","name","status":"error","error":"..."}]}`; `error` key absent for ok.
- `push_if_changed_dedups`: mock-free — assert `last_pushed` logic via two calls
  with a stub `ChatClient`? ChatClient can't be mocked easily; instead test the
  pure parts (`payload_json`, `record_startup`) and cover the push path in the
  integration test below.

**`tests/registration_test.rs`** (or a new `state_report_test.rs`) — integration
against `start_test_server()` from `tests/common/mod.rs`:

1. Connect a plain `ChatClient`, `subscribe_plugin_states({states: []})` +
   `on_plugin_state_event`.
2. Run `run_plugin` in a spawned task with a config of one good stub + one bad
   cmd (extend `common::config_with` into a `config_multi(&[(name, cmd)])`
   helper).
3. Assert the received `pluginStateChanged` carries `schema: "mcpStatus:1"`,
   `format: json`, `key: "status"`, and the payload lists the bad server with
   `status: "error"` and the stub with `"ok"`.
4. Trigger a `stub:fail` tool call (existing tool_call_test pattern) → assert a
   second event with the stub now `"error"`; then `stub:echo` → recovery event.

### 7. `plugins/rhd_plugin_mcp/README.md` (modify)

- Startup section: per-server best-effort; failures surface in the `mcpStatus:1`
  state instead of killing the plugin (config parse errors still fatal).
- New "Status reporting" section: key/format/schema, payload example, when
  pushes happen (startup, tool-call transport failure, recovery), dedup rule.

## Tests

```bash
cargo test -p rhd_plugin_mcp
mise run check-cargo
```

## Implementation Notes

1. **Ordering matters**: register the plugin *before* pool startup so a
   total-failure startup still has a connection to push error state through.
2. **Status is per-server, keyed by `id`** (not `name`) — ids are unique per
   config; payload carries both.
3. **Config order in payload** keeps diffs stable (HashMap iteration would
   reorder entries and defeat the dedup).
4. **Tool-level `is_error` ≠ server error**: MCP servers legitimately return
   tool errors; only transport/protocol failures (`PoolError::Call`, spawn,
   list_tools) flip the server status. Prevents false alarms.
5. **Push failures are non-fatal** in steady state (`let _ =` in handlers) —
   a transient WS hiccup must not stop tool execution; the next change retries.
   Only the *initial* push is hard-required (`?` → `PluginError::StatusReport`).
6. **Dedup compares full payload strings** — cheap (few servers) and exact;
   version assignment is the server's job (Phase 2/3).
7. `McpStatusTracker::record_startup` must be `async` (takes the status lock);
   keep all tracker methods lock-disciplined (tokio Mutex, no holding across
   `await` on the network push — `push_if_changed` releases before sending).

## Dependencies

- Depends on: Phases 1, 4 (`update_plugin_state` typed method). Phase 3 for the
  integration test's server side.
- Blocks: nothing (frontend reads via Phase 6+).
