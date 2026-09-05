# Phase 4: Plugin lifecycle, chat gating, and tool registration

## Overview

Implement the full plugin lifecycle in `plugin.rs` plus pure gating predicates in `gating.rs`:

1. Connect (with retry), `registerPlugin`, drain `getPendingAcks`.
2. Create `ChatMonitor`, `subscribe_to_all_chats`.
3. On every chat state change (and once at startup for existing chats), compute which configured servers are **eligible** for that chat and register their prefixed tools via `addTools` — idempotently, tracked per (chat, server).
4. Subscribe to custom events and acknowledge everything (so `ai_completions:preRequest` never times out waiting for this plugin).
5. Keep-alive loop.

Gating rules (AD-2 of the grand plan), evaluated per chat:
- **Worktree rule:** `--worktree W` given → chat must carry tag `worktree:W`; not given → chat must carry **no** `worktree:*` tag.
- **Server rule:** server has `registerOnTag: T` → chat must carry tag `T`; otherwise always eligible.
- Both must pass (AND).

**Scope:**
- In: `gating.rs` (new), `plugin.rs` (replace Phase 2 stub), `lib.rs` module list, `main.rs` wiring already calls `run_plugin`.
- Out: tool-call execution subscription (Phase 5 — this phase leaves a clearly-marked insertion point).

**Depends on:** Phase 2 (config types, CLI), Phase 3 (`McpPool`).

## Files to Create/Modify

### 1. `plugins/rhd_plugin_mcp/src/lib.rs`

```rust
pub mod config;
pub mod gating;
pub mod mcp_pool;
pub mod plugin;
```

### 2. `plugins/rhd_plugin_mcp/src/gating.rs`

**Create** — pure, fully unit-testable:

```rust
//! Chat eligibility predicates (AD-2): worktree gate AND per-server tag gate.

use crate::config::ResolvedServer;

/// Prefix identifying worktree tags on chats: `worktree:<workTreeId>`.
pub const WORKTREE_TAG_PREFIX: &str = "worktree:";

/// True if the chat carries any `worktree:*` tag.
pub fn has_worktree_tag(chat_tags: &[String]) -> bool {
    chat_tags.iter().any(|t| t.starts_with(WORKTREE_TAG_PREFIX))
}

/// Worktree gate:
/// - `Some(w)`: chat must carry the exact tag `worktree:{w}`.
/// - `None`: chat must carry no `worktree:*` tag at all.
pub fn worktree_gate(chat_tags: &[String], worktree: Option<&str>) -> bool {
    match worktree {
        Some(w) => {
            let required = format!("{}{}", WORKTREE_TAG_PREFIX, w);
            chat_tags.iter().any(|t| t == &required)
        }
        None => !has_worktree_tag(chat_tags),
    }
}

/// Per-server gate: a server with `registerOnTag: T` is eligible only for
/// chats carrying the exact tag `T`; servers without the field are always eligible.
pub fn server_gate(chat_tags: &[String], register_on_tag: Option<&str>) -> bool {
    match register_on_tag {
        Some(tag) => chat_tags.iter().any(|t| t == tag),
        None => true,
    }
}

/// Ids of all servers eligible for a chat (AND of both gates).
pub fn eligible_server_ids(
    servers: &[&ResolvedServer],
    chat_tags: &[String],
    worktree: Option<&str>,
) -> Vec<String> {
    if !worktree_gate(chat_tags, worktree) {
        return Vec::new();
    }
    servers
        .iter()
        .filter(|s| server_gate(chat_tags, s.register_on_tag.as_deref()))
        .map(|s| s.id.clone())
        .collect()
}
```

### 3. `plugins/rhd_plugin_mcp/src/plugin.rs`

**Replace the Phase 2 stub entirely:**

```rust
//! Core plugin lifecycle: connect, register, gate chats, register tools.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use rhd_chat_api::{AckCustomEventParams, AddToolsParams, GetPendingAcksParams, RegisterPluginParams};
use rhd_chat_client::{ChatClient, ChatState};
use tokio::sync::RwLock;

use crate::config::PluginConfig;
use crate::gating::eligible_server_ids;
use crate::mcp_pool::McpPool;

/// Per-chat set of server ids whose tools this plugin instance has already
/// registered (idempotency guard, AD-3).
pub type ChatRegistrations = Arc<RwLock<HashMap<i64, HashSet<String>>>>;

/// Run the MCP plugin.
pub async fn run_plugin(
    server_url: &str,
    plugin_id: &str,
    worktree: Option<&str>,
    config: PluginConfig,
) -> Result<(), PluginError> {
    // 1. Spawn all MCP servers first (fail-fast, AD-4).
    let pool = Arc::new(
        McpPool::startup(&config)
            .await
            .map_err(|e| PluginError::Pool(e.to_string()))?,
    );
    tracing::info!("MCP pool ready ({} servers)", pool.server_configs().len());

    // 2. Connect + register as plugin.
    let client = Arc::new(
        ChatClient::connect_with_retry(server_url)
            .await
            .map_err(|e| PluginError::Connection(e.to_string()))?,
    );
    client
        .register_plugin(RegisterPluginParams {
            plugin_id: plugin_id.to_string(),
        })
        .await
        .map_err(|e| PluginError::Registration(e.to_string()))?;
    tracing::info!("Registered as plugin: {}", plugin_id);

    // 3. Drain pending acks (missed while disconnected).
    let pending_acks = client
        .get_pending_acks(GetPendingAcksParams {})
        .await
        .map_err(|e| PluginError::PendingAcks(e.to_string()))?;
    for event in pending_acks.pending_events {
        tracing::info!("Processing pending event: {}", event.event_name);
        client
            .ack_custom_event(AckCustomEventParams {
                event_id: event.event_id,
                is_rejected: None,
            })
            .await
            .map_err(|e| PluginError::PendingAcks(e.to_string()))?;
    }

    // 4. Chat monitor over all chats.
    let chat_monitor = Arc::new(
        client
            .create_chat_monitor()
            .await
            .map_err(|e| PluginError::MonitorCreate(e.to_string()))?,
    );
    chat_monitor
        .subscribe_to_all_chats()
        .await
        .map_err(|e| PluginError::Subscription(e.to_string()))?;
    tracing::info!("Subscribed to all chats");

    let registrations: ChatRegistrations = Arc::new(RwLock::new(HashMap::new()));

    // 5. Event-driven registration on chat state changes.
    {
        let client_c = Arc::clone(&client);
        let pool_c = Arc::clone(&pool);
        let regs_c = Arc::clone(&registrations);
        let worktree_c = worktree.map(|s| s.to_string());

        chat_monitor
            .on_chat_state_change(move |chat_id, state| {
                let client = Arc::clone(&client_c);
                let pool = Arc::clone(&pool_c);
                let regs = Arc::clone(&regs_c);
                let worktree = worktree_c.clone();

                // Callbacks are sync Fn — spawn the async work (never block
                // the monitor/event task; see memory/development.md pattern).
                tokio::spawn(async move {
                    if let Err(e) =
                        register_missing_tools(&client, &pool, &regs, worktree.as_deref(), &state)
                            .await
                    {
                        tracing::error!(chat_id = chat_id, error = %e, "tool registration failed");
                    }
                });
            })
            .await;
    }

    // 6. Startup reconciliation: existing chats may already be eligible.
    for chat_id in chat_monitor.get_chat_ids().await {
        if let Some(state) = chat_monitor.get_chat_state(chat_id).await {
            if let Err(e) =
                register_missing_tools(&client, &pool, &registrations, worktree, &state).await
            {
                tracing::error!(chat_id, error = %e, "startup tool registration failed");
            }
        }
    }
    tracing::info!("Startup reconciliation complete");

    // 7. Acknowledge all custom events (AD-8) so preRequest coordination
    //    never blocks on this plugin.
    let client_for_events = Arc::clone(&client);
    let _custom_event_token = client.on_custom_event(move |event| {
        let client = Arc::clone(&client_for_events);
        async move {
            tracing::debug!(
                event_id = %event.event_id,
                event_name = %event.event_name,
                "acknowledging custom event"
            );
            let _ = client
                .ack_custom_event(AckCustomEventParams {
                    event_id: event.event_id,
                    is_rejected: None,
                })
                .await;
        }
    });

    // 8. Phase 5 inserts the tool-call subscription here:
    //    client.on_tool_call(0, pool.all_tool_names(), handler)

    // 9. Keep the plugin running.
    tracing::info!("Plugin running in event-driven mode");
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

/// Register tools of all eligible-but-not-yet-registered servers for one chat.
///
/// Idempotent via `registrations`; a chat that gains tags later picks up the
/// remaining servers' tools (AD-3). Never unregisters.
pub async fn register_missing_tools(
    client: &ChatClient,
    pool: &McpPool,
    registrations: &ChatRegistrations,
    worktree: Option<&str>,
    state: &ChatState,
) -> Result<(), PluginError> {
    let eligible = eligible_server_ids(&pool.server_configs(), &state.tags, worktree);
    if eligible.is_empty() {
        return Ok(());
    }

    let already: HashSet<String> = registrations
        .read()
        .await
        .get(&state.chat_id)
        .cloned()
        .unwrap_or_default();

    let new_ids: Vec<String> = eligible
        .into_iter()
        .filter(|id| !already.contains(id))
        .collect();
    if new_ids.is_empty() {
        return Ok(());
    }

    let mut tools = Vec::new();
    for id in &new_ids {
        tools.extend(pool.tools_for_server(id).iter().cloned());
    }

    client
        .add_tools(AddToolsParams {
            chat_id: state.chat_id,
            tools,
        })
        .await
        .map_err(|e| PluginError::AddTools(e.to_string()))?;

    let mut regs = registrations.write().await;
    let set = regs.entry(state.chat_id).or_default();
    set.extend(new_ids.iter().cloned());

    tracing::info!(
        chat_id = state.chat_id,
        servers = ?new_ids,
        "registered MCP tools on chat"
    );
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("failed to start MCP pool: {0}")]
    Pool(String),
    #[error("failed to connect to server: {0}")]
    Connection(String),
    #[error("failed to register plugin: {0}")]
    Registration(String),
    #[error("pending acks error: {0}")]
    PendingAcks(String),
    #[error("failed to create monitor: {0}")]
    MonitorCreate(String),
    #[error("failed to subscribe: {0}")]
    Subscription(String),
    #[error("failed to add tools: {0}")]
    AddTools(String),
}
```

## Integration points with existing code

- `ChatMonitor::on_chat_state_change` fires on `chatCreated` (with initial tags), `chatUpdated` (tag changes), and per-chat message/queue events (state refresh). Tag changes therefore re-trigger eligibility evaluation automatically.
- `ChatState` is re-exported from `rhd_chat_client` (used by `rhd_plugin_system_prompt` the same way).
- `addTools` associates tools with the calling plugin id server-side (`ToolInfo.pluginId` in `getTools`), so cleanup/dedup semantics match other plugins.

## Tests

Authored in Phase 6:
- `gating.rs` truth table (worktree on/off × `worktree:*` presence × `registerOnTag` match/mismatch × AND composition).
- `register_missing_tools` against the in-process test server: eligible chat → `getTools` shows prefixed names with `pluginId == "mcp"`; ineligible chat → unchanged; tag added later → remaining servers registered; second call → no duplicate work.

## Implementation Notes

1. **Pool before connect:** if any MCP server fails to spawn, the plugin exits before registering — no half-alive plugin holding chats hostage in ack coordination.
2. **Why a per-chat `HashSet<server_id>` instead of scanning existing tools:** cheaper on the hot path (state-change callbacks fire for every message event) and avoids parsing `getTools` per chat. Plugin restart re-registers the same definitions; `addTools` is an upsert keyed by (chat, plugin, tool name), so duplicates are harmless (verify during Phase 6 with a restart-style test).
3. **No deregistration on tag removal** (AD-3): in-flight conversations keep their tools; execution in Phase 5 checks the registration set, not live tags.
4. **`worktree` is `Option<String>` cloned per callback** because the monitor callback is `Fn` + `'static`; `Option<&str>` cannot be captured across `tokio::spawn`.
5. **Custom events:** we handle none, but MUST ack all (plugins/README.md "Acknowledging Unhandled Events"). The `on_custom_event` callback is dispatched via `tokio::spawn` by the client read loop, so awaiting `ack_custom_event` inside is safe (read task not blocked).
6. **Step 8 marker** is where Phase 5 plugs in; keep numbering/comments intact.
7. **Bind subscription tokens, never drop them:** `CancellationToken::drop` signals cancellation and `dispatch_event` prunes terminated subscriptions on every event. Existing plugins rely on the quirk that the cancel value is never received (so `is_terminated()` stays false) — do not replicate that fragility. `_custom_event_token` (and Phase 5's `_tool_call_token`) must stay in scope until the keep-alive loop.

## Dependencies

- Requires Phases 2 and 3.
- Required by Phase 5 (`ChatRegistrations`, pool, subscription insertion point).
