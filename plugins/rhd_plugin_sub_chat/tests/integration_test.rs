//! Integration tests: the sub-chat plugin registers all three tools in
//! every chat (per-chat registration on `chatCreated`).

use std::time::Duration;

use rhd_chat_api::{CreateChatParams, GetToolsParams};
use rhd_chat_client::ChatClient;
use tokio::time::{sleep, timeout};

const EXPECTED_TOOLS: [&str; 3] = ["rhd_sub_chat", "rhd_sub_chat_status", "rhd_sub_chat_await"];

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

/// Wait until all three sub-chat tools are registered for `chat_id`.
async fn wait_for_tools(client: &ChatClient, chat_id: i64) {
    let result = timeout(Duration::from_secs(10), async {
        loop {
            let names: Vec<String> = client
                .get_tools(GetToolsParams { chat_id })
                .await
                .expect("getTools failed")
                .tools
                .into_iter()
                .map(|t| t.tool.function.name)
                .collect();
            if EXPECTED_TOOLS.iter().all(|t| names.iter().any(|n| n == t)) {
                return;
            }
            sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(result.is_ok(), "chat {chat_id} never got all three sub-chat tools");
}

/// Every chat created while the plugin runs ends up with exactly the three
/// sub-chat tools, registered under the plugin's id.
#[tokio::test]
async fn test_registers_three_tools_in_every_chat() {
    let port = start_test_server().await;
    let url = format!("ws://127.0.0.1:{}/", port);

    // Start the plugin before the chats exist.
    let plugin_url = url.clone();
    let plugin_handle = tokio::spawn(async move {
        rhd_plugin_sub_chat::plugin::run_plugin(&plugin_url, "rhd_plugin_sub_chat").await
    });
    sleep(Duration::from_millis(500)).await;
    assert!(!plugin_handle.is_finished(), "plugin must keep running");

    let client = ChatClient::connect(&url)
        .await
        .expect("client connect failed");

    // First chat gets the tools...
    let chat_a = client
        .create_chat(CreateChatParams {
            title: "a".to_string(),
            tags: vec![],
        })
        .await
        .expect("create chat a failed")
        .chat_id;
    wait_for_tools(&client, chat_a).await;

    // ...and so does a second, later chat.
    let chat_b = client
        .create_chat(CreateChatParams {
            title: "b".to_string(),
            tags: vec![],
        })
        .await
        .expect("create chat b failed")
        .chat_id;
    wait_for_tools(&client, chat_b).await;

    // Both chats carry exactly the three tools, owned by our plugin id.
    let mut expected_sorted = EXPECTED_TOOLS.to_vec();
    expected_sorted.sort();
    for chat_id in [chat_a, chat_b] {
        let tools = client
            .get_tools(GetToolsParams { chat_id })
            .await
            .expect("getTools failed")
            .tools;
        let mut names: Vec<_> = tools
            .iter()
            .map(|t| t.tool.function.name.clone())
            .collect();
        names.sort();
        assert_eq!(names, expected_sorted, "unexpected tool set for chat {chat_id}");
        assert!(
            tools
                .iter()
                .all(|t| t.plugin_id == "rhd_plugin_sub_chat"),
            "tools must be registered under the plugin id (chat {chat_id})"
        );
    }

    plugin_handle.abort();
}
