//! Integration tests for tool registration gating (eligibility tags, worktree filter).

mod common;

use std::sync::Arc;

use rhd_chat_api::{CreateChatParams, GetToolsParams, RegisterPluginParams};
use rhd_plugin_mcp::mcp_pool::McpPool;
use rhd_plugin_mcp::plugin::register_missing_tools;

use common::{chat_state, config_with, connect_client, new_regs, start_test_server};

#[tokio::test]
async fn registers_tools_only_on_eligible_chats() {
    let (port, _h) = start_test_server().await;
    let client = connect_client(port).await;
    client
        .register_plugin(RegisterPluginParams {
            plugin_id: "mcp".into(),
        })
        .await
        .unwrap();
    let pool = Arc::new(
        McpPool::startup(&config_with("stub", Some("mcp:common")))
            .await
            .unwrap(),
    );
    let regs = new_regs();

    // Ineligible chat (no tag): nothing registered.
    let chat_a = client
        .create_chat(CreateChatParams {
            title: "a".into(),
            tags: vec![],
        })
        .await
        .unwrap()
        .chat_id;
    register_missing_tools(&client, &pool, &regs, None, &chat_state(chat_a, vec![]))
        .await
        .unwrap();
    assert!(client
        .get_tools(GetToolsParams { chat_id: chat_a })
        .await
        .unwrap()
        .tools
        .is_empty());

    // Eligible chat: prefixed tool registered under this plugin id.
    let chat_b = client
        .create_chat(CreateChatParams {
            title: "b".into(),
            tags: vec!["mcp:common".into()],
        })
        .await
        .unwrap()
        .chat_id;
    register_missing_tools(
        &client,
        &pool,
        &regs,
        None,
        &chat_state(chat_b, vec!["mcp:common".into()]),
    )
    .await
    .unwrap();
    let tools = client
        .get_tools(GetToolsParams { chat_id: chat_b })
        .await
        .unwrap()
        .tools;
    assert_eq!(tools.len(), 2);
    assert!(tools.iter().all(|t| t.plugin_id == "mcp"));
    let mut names: Vec<_> = tools
        .iter()
        .map(|t| t.tool.function.name.clone())
        .collect();
    names.sort();
    assert_eq!(names, vec!["stub:echo", "stub:fail"]);

    // Idempotent: second pass adds nothing.
    register_missing_tools(
        &client,
        &pool,
        &regs,
        None,
        &chat_state(chat_b, vec!["mcp:common".into()]),
    )
    .await
    .unwrap();
    assert_eq!(
        client
            .get_tools(GetToolsParams { chat_id: chat_b })
            .await
            .unwrap()
            .tools
            .len(),
        2
    );
}

#[tokio::test]
async fn worktree_filter_blocks_and_admits_chats() {
    let (port, _h) = start_test_server().await;
    let client = connect_client(port).await;
    client
        .register_plugin(RegisterPluginParams {
            plugin_id: "mcp".into(),
        })
        .await
        .unwrap();
    let pool = Arc::new(McpPool::startup(&config_with("stub", None)).await.unwrap());
    let regs = new_regs();

    let chat = client
        .create_chat(CreateChatParams {
            title: "w".into(),
            tags: vec!["worktree:W2".into()],
        })
        .await
        .unwrap()
        .chat_id;

    // Plugin bound to W1: W2 chat ineligible.
    register_missing_tools(
        &client,
        &pool,
        &regs,
        Some("W1"),
        &chat_state(chat, vec!["worktree:W2".into()]),
    )
    .await
    .unwrap();
    assert!(client
        .get_tools(GetToolsParams { chat_id: chat })
        .await
        .unwrap()
        .tools
        .is_empty());

    // W1 chat eligible.
    let chat_w1 = client
        .create_chat(CreateChatParams {
            title: "w1".into(),
            tags: vec!["worktree:W1".into()],
        })
        .await
        .unwrap()
        .chat_id;
    register_missing_tools(
        &client,
        &pool,
        &regs,
        Some("W1"),
        &chat_state(chat_w1, vec!["worktree:W1".into()]),
    )
    .await
    .unwrap();
    assert_eq!(
        client
            .get_tools(GetToolsParams { chat_id: chat_w1 })
            .await
            .unwrap()
            .tools
            .len(),
        2
    );

    // Unbound plugin must skip worktree-tagged chats entirely.
    let regs2 = new_regs();
    register_missing_tools(
        &client,
        &pool,
        &regs2,
        None,
        &chat_state(chat_w1, vec!["worktree:W1".into()]),
    )
    .await
    .unwrap();
    assert!(regs2
        .read()
        .await
        .get(&chat_w1)
        .map(|s| s.is_empty())
        .unwrap_or(true));
}
