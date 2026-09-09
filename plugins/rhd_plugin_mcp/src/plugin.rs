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
use crate::status::McpStatusTracker;
use crate::tool_handler;

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
    // 1. Connect + register as plugin FIRST — state must be pushable even if
    //    every MCP server fails to start.
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

    // 2. Best-effort pool startup; build tracker; push initial state.
    let (pool, reports) = McpPool::startup(&config).await;
    let pool = Arc::new(pool);
    tracing::info!(
        "MCP pool ready ({} healthy of {} configured servers)",
        pool.server_configs().len(),
        reports.len()
    );

    let tracker = Arc::new(McpStatusTracker::new(pool.server_ids()));
    tracker.record_startup(&reports).await;
    tracker
        .push_if_changed(&client)
        .await
        .map_err(|e| PluginError::StatusReport(e.to_string()))?;
    tracing::info!("MCP status state pushed ({} servers)", reports.len());

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

    // 8. Execute tool calls for the prefixed names we registered.
    //    `_tool_call_token` must stay alive until the keep-alive loop:
    //    CancellationToken::drop cancels the subscription (Phase 4 note 7).
    let client_for_tools = Arc::clone(&client);
    let pool_for_tools = Arc::clone(&pool);
    let regs_for_tools = Arc::clone(&registrations);
    let tracker_for_tools = Arc::clone(&tracker);

    let _tool_call_token = client.on_tool_call(0, pool.all_tool_names(), move |event| {
        let client = Arc::clone(&client_for_tools);
        let pool = Arc::clone(&pool_for_tools);
        let regs = Arc::clone(&regs_for_tools);
        let tracker = Arc::clone(&tracker_for_tools);

        async move {
            tracing::debug!(
                chat_id = event.chat_id,
                tool_count = event.message.tool_calls.len(),
                "received MCP tool call event"
            );
            tool_handler::handle_tool_calls(client, pool, regs, tracker, event).await;
        }
    });
    tracing::info!("Subscribed to MCP tool calls");

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
    #[error("status reporting failed: {0}")]
    StatusReport(String),
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
