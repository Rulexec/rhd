//! Main plugin logic.

use std::sync::Arc;
use std::time::Duration;

use rhd_chat_api::{
    AckCustomEventParams, AddToolsParams, GetPendingAcksParams, RegisterPluginParams,
    ToolDefinition,
};
use rhd_chat_client::ChatClient;
use tokio::sync::RwLock;

use crate::handler::{self, HandlerCtx};
use crate::recovery;
use crate::reply::AnswerGuards;
use crate::templates::Templates;
use crate::watcher::{self, Watcher};

/// Run the sub-chat plugin.
///
/// Lifecycle:
/// 1. Load embedded tool-definition templates
/// 2. Connect to chat server
/// 3. Register as plugin
/// 4. Process pending acks
/// 5. Subscribe to custom events and acknowledge them unhandled (the plugin
///    reacts to none yet — never acking would park senders' coordination
///    waits, e.g. `ai_completions:preRequest` / `ai_completions:preDrainQueue`,
///    which would then stall this plugin's own subchats)
/// 6. Create chat monitor, subscribe to all chats
/// 7. On every chat state change: register the three sub-chat tools once per chat
/// 8. Subscribe to tool calls for the three tool names and handle
///    `rhd_sub_chat`: validate → two-phase activate the subchat (create
///    paused → reconcile queue → unpause) → async calls answer with the
///    background notice; sync calls register a watcher waiter and stay
///    parked until the subchat completes. All answers pass through the
///    shared `AnswerGuards`. The status/await names are subscribed already
///    but only dispatched from Phase 5 on — their events fall through in the
///    handler meanwhile.
/// 9. Install the completion watcher's state-change hook (single callback,
///    fans out to every parked waiter of the completed subchat).
/// 10. Run the startup recovery pass (Phase 6, AD-7): re-drive every
///    unfinished sub-chat tool call found in chat history through the very
///    same flows the live handlers use, claims serialized via the shared
///    `AnswerGuards`. A failed pass is logged, never fatal (idempotence
///    gives the next restart the rest).
/// 11. Main loop (keep alive)
pub async fn run_plugin(server_url: &str, plugin_id: &str) -> Result<(), PluginError> {
    let templates = Arc::new(Templates::load().map_err(|e| PluginError::Template(e.to_string()))?);
    tracing::info!("Templates loaded");

    let client = Arc::new(
        ChatClient::connect_with_retry(server_url)
            .await
            .map_err(|e| PluginError::Connection(e.to_string()))?,
    );
    tracing::info!("Connected to chat server");

    client
        .register_plugin(RegisterPluginParams {
            plugin_id: plugin_id.to_string(),
        })
        .await
        .map_err(|e| PluginError::Registration(e.to_string()))?;
    tracing::info!("Registered as plugin: {}", plugin_id);

    // Drain pending acks so other plugins' coordination is never blocked by us.
    let pending_acks = client
        .get_pending_acks(GetPendingAcksParams {})
        .await
        .map_err(|e| PluginError::PendingAcks(e.to_string()))?;
    tracing::info!("Found {} pending acks", pending_acks.pending_events.len());

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

    // Acknowledge every live custom event. The sub-chat plugin reacts to none
    // of them yet, but senders (e.g. ai_completions on `preRequest` /
    // `preDrainQueue`) wait for an ack from every registered plugin — a
    // missing ack here would stall the coordination timeout, and once
    // subchats exist it would park this plugin's own chats. See
    // plugins/README.md, "Acknowledging Unhandled Events".
    let client_for_events = Arc::clone(&client);
    let _custom_event_token = client.on_custom_event(move |event| {
        let client = Arc::clone(&client_for_events);
        async move {
            tracing::debug!(
                event_id = %event.event_id,
                event_name = %event.event_name,
                "acknowledging unhandled custom event"
            );
            let _ = client
                .ack_custom_event(AckCustomEventParams {
                    event_id: event.event_id,
                    is_rejected: None,
                })
                .await;
        }
    });

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

    // Chats we already registered the tools for (avoids redundant addTools calls).
    let initialized_chats = Arc::new(RwLock::new(std::collections::HashSet::new()));

    let client_for_chat = Arc::clone(&client);
    let templates_for_chat = Arc::clone(&templates);
    let initialized_chats_for_chat = Arc::clone(&initialized_chats);

    chat_monitor
        .on_chat_state_change(move |chat_id, _chat_state| {
            let client = Arc::clone(&client_for_chat);
            let templates = Arc::clone(&templates_for_chat);
            let initialized_chats = Arc::clone(&initialized_chats_for_chat);

            // NEVER await inline in the monitor's dispatch path — spawn (see
            // memory/development.md "Event Callbacks Must Not Block").
            tokio::spawn(async move {
                {
                    let initialized = initialized_chats.read().await;
                    if initialized.contains(&chat_id) {
                        return;
                    }
                }

                let tools: Vec<ToolDefinition> = match parse_tool_definitions(&templates) {
                    Ok(tools) => tools,
                    Err(e) => {
                        tracing::error!(
                            chat_id = chat_id,
                            error = %e,
                            "failed to parse tool definitions"
                        );
                        return;
                    }
                };

                tracing::info!(chat_id = chat_id, "registering sub-chat tools");
                if let Err(e) = client.add_tools(AddToolsParams { chat_id, tools }).await {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %e,
                        "failed to register tools"
                    );
                    return; // retry on the next state change for this chat
                }

                let mut initialized = initialized_chats.write().await;
                initialized.insert(chat_id);
                tracing::info!(chat_id = chat_id, "sub-chat tools registered");
            });
        })
        .await;

    // The shared answer-dedup guards and the completion watcher are built once
    // here: every answer path (spawn handler, watcher drain, Phase 6 recovery)
    // converges on them, which is exactly where duplicate answers die.
    let guards = Arc::new(AnswerGuards::default());
    let watcher = Arc::new(Watcher::new(Arc::clone(&client), Arc::clone(&guards)));

    // Subscribe once with ALL three tool names even though only rhd_sub_chat
    // is dispatched yet (Phase 5 adds the status/await arms without touching
    // this subscription). The client dispatcher runs callbacks in spawned
    // tasks, so awaiting client requests inside the handler is safe.
    // Keep the token bound for the plugin's lifetime — dropping it cancels
    // the subscription.
    let ctx = HandlerCtx {
        client: Arc::clone(&client),
        watcher: Arc::clone(&watcher),
        guards,
    };
    // Recovery runs with the same shared pieces (watcher + guards) as the
    // live subscription; the Arcs make the two paths converge on exactly
    // one set of dedup state.
    let recovery_ctx = ctx.clone();
    let _tool_call_token = client.on_tool_call(
        0, // wildcard: all chats (subchats included — recursion is by design)
        vec![
            "rhd_sub_chat".to_string(),
            "rhd_sub_chat_status".to_string(),
            "rhd_sub_chat_await".to_string(),
        ],
        move |event| {
            let ctx = ctx.clone();
            async move {
                handler::handle_tool_call_event(ctx, event).await;
            }
        },
    );

    // Single ChatMonitor hook fanning into the watcher: when a watched
    // subchat's state satisfies the completion predicate, all its parked
    // waiters are answered with the final assistant content (AD-1/AD-2).
    // Installed before the keep-alive loop; waiters registered earlier are
    // covered by Watcher::register_or_complete's own immediate check.
    watcher::install(&chat_monitor, Arc::clone(&watcher)).await;
    tracing::info!("Completion watcher installed");

    // Startup recovery (AD-7, Phase 6): re-drive every unfinished sub-chat
    // tool call from chat history alone. Deliberately AFTER the live
    // subscriptions and the watcher hook: state changes it causes (unpause,
    // queueing) flow through the live hooks, and a completion landing
    // mid-scan is caught by `register_or_complete`'s evaluate-after-insert.
    // Concurrent live handling is serialized by the AnswerGuards claim set.
    // A failed pass must NEVER abort the plugin: parked calls simply stay
    // parked and the next restart re-drives them (everything is idempotent).
    match recovery::run(Arc::clone(&client), chat_monitor.as_ref(), recovery_ctx).await {
        Ok(report) => tracing::info!(?report, "startup recovery complete"),
        Err(e) => tracing::error!(
            error = %e,
            "startup recovery failed — some subchat calls may stay parked"
        ),
    }

    tracing::info!("Plugin running in event-driven mode");
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

/// Parse the three embedded tool-definition templates into protocol types.
fn parse_tool_definitions(templates: &Templates) -> Result<Vec<ToolDefinition>, serde_json::Error> {
    Ok(vec![
        serde_json::from_str(templates.spawn_definition())?,
        serde_json::from_str(templates.status_definition())?,
        serde_json::from_str(templates.await_definition())?,
    ])
}

/// Errors that can occur during plugin execution.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("failed to load templates: {0}")]
    Template(String),
    #[error("failed to connect to server: {0}")]
    Connection(String),
    #[error("failed to register plugin: {0}")]
    Registration(String),
    #[error("failed to get pending acks: {0}")]
    PendingAcks(String),
    #[error("failed to create monitor: {0}")]
    MonitorCreate(String),
    #[error("failed to subscribe: {0}")]
    Subscription(String),
}
