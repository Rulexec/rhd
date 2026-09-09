//! Integration: `run_plugin` publishes `mcpStatus:1` state and updates it
//! when a server fails or recovers — driven through the real event pipeline.

mod common;

use std::time::Duration;

use rhd_chat_api::{
    AddMessageParams, CreateChatParams, FunctionCall, GetToolsParams, PluginState,
    StateFormat, SubscribePluginStatesParams, ToolCall, UpdateMessageParams,
};
use rhd_chat_client::{ChatClient, PluginStateEvent};
use serde_json::Value;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use rhd_plugin_mcp::plugin::run_plugin;

use common::{config_multi, connect_client, start_test_server, stub_cmd, tool_messages};

const BAD_CMD: &str = "/nonexistent/mcp-binary";
const WAIT: Duration = Duration::from_secs(15);

fn status_of(content: &Value, server_id: &str) -> Option<String> {
    content["mcp"]
        .as_array()?
        .iter()
        .find(|e| e["id"] == server_id)
        .and_then(|e| e["status"].as_str())
        .map(|s| s.to_string())
}

fn error_of(content: &Value, server_id: &str) -> Option<String> {
    content["mcp"]
        .as_array()?
        .iter()
        .find(|e| e["id"] == server_id)
        .and_then(|e| e["error"].as_str())
        .map(|s| s.to_string())
}

/// Next `pluginStateChanged` whose parsed content satisfies `want`.
async fn next_matching_state(
    rx: &mut UnboundedReceiver<PluginState>,
    want: impl Fn(&Value) -> bool,
) -> (PluginState, Value) {
    loop {
        let state = tokio::time::timeout(WAIT, rx.recv())
            .await
            .expect("timed out waiting for pluginStateChanged")
            .expect("state channel closed");
        assert_eq!(state.key, "status");
        assert_eq!(state.schema, "mcpStatus:1");
        assert_eq!(state.format, StateFormat::Json);
        let content: Value = serde_json::from_str(&state.content).unwrap();
        if want(&content) {
            return (state, content);
        }
    }
}

/// Add an assistant message and finalize it with one tool call — the
/// `assistantMessageWithToolCalls` event path the plugin listens to.
async fn trigger_tool_call(client: &ChatClient, chat_id: i64, tool: &str, args: &str, call_id: &str) {
    let added = client
        .add_message(AddMessageParams {
            chat_id,
            role: "assistant".to_string(),
            content: String::new(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
            is_finished: false,
            is_streaming: false,
        })
        .await
        .unwrap();
    let tool_calls = serde_json::to_string(&vec![ToolCall {
        id: call_id.to_string(),
        call_type: "function".to_string(),
        function: FunctionCall {
            name: tool.to_string(),
            arguments: args.to_string(),
        },
        tags: vec![],
    }])
    .unwrap();
    client
        .update_message(UpdateMessageParams {
            message_id: added.message_id,
            content: None,
            reasoning_content: None,
            role: None,
            add_tags: vec![],
            remove_tags: vec![],
            is_finished: Some(true),
            is_streaming: None,
            tool_calls: Some(tool_calls),
        })
        .await
        .unwrap();
}

/// Poll until the plugin answered the call with a `tool`-role message.
async fn wait_for_tool_answer(client: &ChatClient, chat_id: i64, call_id: &str) {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let answered = tool_messages(client, chat_id)
            .await
            .iter()
            .any(|m| m.tool_call_id.as_deref() == Some(call_id));
        if answered {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "tool call {call_id} was never answered"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn run_plugin_reports_mcp_status_state() {
    let (port, _server) = start_test_server().await;
    let url = format!("ws://127.0.0.1:{}/", port);

    // Observer: subscribed to plugin states BEFORE the plugin starts pushing.
    let observer = connect_client(port).await;
    observer
        .subscribe_plugin_states(SubscribePluginStatesParams { states: vec![] })
        .await
        .unwrap();
    let (tx, mut rx) = unbounded_channel::<PluginState>();
    let _state_token = observer.on_plugin_state_event(move |event| {
        let tx = tx.clone();
        async move {
            if let PluginStateEvent::Changed(data) = event {
                let _ = tx.send(data.state);
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // One healthy stub + one unspawnable server; the plugin must survive.
    let config = config_multi(&[("stub", &stub_cmd()), ("bad", BAD_CMD)]);
    let plugin_url = url.clone();
    let plugin_task = tokio::spawn(async move { run_plugin(&plugin_url, "mcp", None, config).await });

    // 1. Initial push: stub ok, bad error — despite the partial failure.
    let (state, content) = next_matching_state(&mut rx, |c| {
        status_of(c, "stub").as_deref() == Some("ok")
            && status_of(c, "bad").as_deref() == Some("error")
    })
    .await;
    assert_eq!(state.plugin_id, "mcp");
    assert_eq!(state.version, 1);
    let bad_error = error_of(&content, "bad").expect("bad server must carry an error message");
    assert!(
        bad_error.starts_with("spawn/initialize failed:"),
        "unexpected bad-server error: {bad_error}"
    );
    assert!(error_of(&content, "stub").is_none(), "ok entry omits error");

    // 2. Drive a failing tool call through the real event pipeline.
    let driver = connect_client(port).await;
    let chat_id = driver
        .create_chat(CreateChatParams {
            title: "status".to_string(),
            tags: vec![],
        })
        .await
        .unwrap()
        .chat_id;

    // Wait until the plugin registered the stub tools on this chat.
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        let tools = driver
            .get_tools(GetToolsParams { chat_id })
            .await
            .unwrap()
            .tools;
        if tools.len() == 2 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "plugin never registered stub tools on the chat"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    trigger_tool_call(&driver, chat_id, "stub:fail", "{}", "call_fail").await;
    wait_for_tool_answer(&driver, chat_id, "call_fail").await;

    let (_state, content) =
        next_matching_state(&mut rx, |c| status_of(c, "stub").as_deref() == Some("error"))
            .await;
    let stub_error = error_of(&content, "stub").expect("failed stub must carry an error message");
    assert!(
        stub_error.contains("tool call failed:"),
        "unexpected stub error: {stub_error}"
    );
    // The broken server is still reported as error.
    assert_eq!(status_of(&content, "bad").as_deref(), Some("error"));

    // 3. A later successful call flips the server back to ok.
    trigger_tool_call(
        &driver,
        chat_id,
        "stub:echo",
        r#"{"input":"hi"}"#,
        "call_ok",
    )
    .await;
    wait_for_tool_answer(&driver, chat_id, "call_ok").await;

    let (_state, content) =
        next_matching_state(&mut rx, |c| status_of(c, "stub").as_deref() == Some("ok")).await;
    assert!(error_of(&content, "stub").is_none());

    plugin_task.abort();
}
