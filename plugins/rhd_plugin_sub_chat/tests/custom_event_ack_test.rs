//! Regression test: the sub-chat plugin must acknowledge custom events.
//!
//! The plugin reacts to no custom events (yet), but it MUST ack them: senders
//! (e.g. `ai_completions:preRequest`, `ai_completions:preDrainQueue`) wait
//! for an ack from every registered plugin in their monitor snapshot, and a
//! silent plugin parks chats for the full 30 s timeout before erroring them —
//! which would also stall this plugin's own subchats.
//!
//! The test below drives the exact production coordination primitive:
//! a sender connection emits a custom event and awaits acks via
//! `PluginsMonitor::wait_for_acks_except`.

use std::time::Duration;

use rhd_chat_api::{RegisterPluginParams, SendCustomEventParams};
use rhd_chat_client::ChatClient;
use tokio::time::{sleep, timeout};

/// Start an ephemeral-port, in-memory chat server.
async fn start_test_server() -> u16 {
    let chat_config = rhd_chat_server::config::Config {
        host: "127.0.0.1".to_string(),
        port: 0,
        db_path: ":memory:".to_string(),
        clear_pending_acks: false,
    };
    let (port, _server_handle) = rhd_chat_server::server::start(chat_config)
        .await
        .expect("Failed to start chat server");
    port
}

/// The sub-chat plugin acknowledges live custom events, so an all-plugin
/// ack-wait (as used by ai_completions) completes instead of parking.
#[tokio::test]
async fn test_sub_chat_acks_live_custom_events() {
    let port = start_test_server().await;
    let url = format!("ws://127.0.0.1:{}/", port);

    // 1. Start the sub-chat plugin FIRST: the sender's PluginsMonitor snapshots
    //    the registered-plugin list at creation, so the plugin must already be
    //    registered to land in the expected-ack set.
    let plugin_url = url.clone();
    let plugin_handle = tokio::spawn(async move {
        rhd_plugin_sub_chat::plugin::run_plugin(&plugin_url, "rhd_plugin_sub_chat").await
    });
    sleep(Duration::from_millis(500)).await;
    assert!(
        !plugin_handle.is_finished(),
        "sub-chat plugin must keep running"
    );

    // 2. Connect as a sender plugin and wait until the sub-chat plugin is visible.
    let sender = ChatClient::connect(&url)
        .await
        .expect("sender connect failed");
    sender
        .register_plugin(RegisterPluginParams {
            plugin_id: "sender".to_string(),
        })
        .await
        .expect("sender register failed");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let plugins = sender
            .get_plugins(rhd_chat_api::GetPluginsParams {})
            .await
            .expect("getPlugins failed")
            .plugins;
        if plugins.iter().any(|p| p.plugin_id == "rhd_plugin_sub_chat") {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "sub-chat plugin never became visible via getPlugins"
        );
        sleep(Duration::from_millis(50)).await;
    }

    // 3. Create the monitor AFTER the plugin registered (snapshot semantics),
    //    emit an event exactly like ai_completions does, and await acks.
    let monitor = sender
        .create_plugins_monitor()
        .await
        .expect("create_plugins_monitor failed");
    let event = sender
        .send_custom_event(SendCustomEventParams {
            event_name: "ai_completions:preRequest".to_string(),
            additional: Some(r#"{"triggerReason":"none"}"#.to_string()),
            chat_id: None,
            message_id: None,
            tool_call_id: None,
        })
        .await
        .expect("sendCustomEvent failed");

    // 4. The plugin must ack promptly; the short bound makes the old behavior
    //    (30 s sender timeout) fail this test.
    timeout(
        Duration::from_secs(5),
        monitor.wait_for_acks_except(&event.event_id, &["sender"], Duration::from_secs(4)),
    )
    .await
    .expect("sub-chat plugin never acknowledged the live custom event")
    .expect("sub-chat plugin never acknowledged the live custom event");

    plugin_handle.abort();
}
