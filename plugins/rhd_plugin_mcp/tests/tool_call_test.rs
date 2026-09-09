//! Integration tests for MCP tool-call execution and result push-back.

mod common;

use std::sync::Arc;

use rhd_plugin_mcp::tool_handler;

use common::{new_regs, setup_registered_chat, tool_call_event, tool_messages};

#[tokio::test]
async fn tool_call_roundtrip_pushes_result_and_dedups() {
    let env = setup_registered_chat(None, vec![]).await;

    tool_handler::handle_tool_calls(
        Arc::clone(&env.client),
        Arc::clone(&env.pool),
        Arc::clone(&env.regs),
        Arc::clone(&env.tracker),
        tool_call_event(env.chat_id, "stub:echo", r#"{"input":"hello"}"#, "call_1"),
    )
    .await;

    let results = tool_messages(&env.client, env.chat_id).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool_call_id.as_deref(), Some("call_1"));
    assert_eq!(results[0].content, "echo: hello");

    // Duplicate guard: replaying the same event adds nothing.
    tool_handler::handle_tool_calls(
        Arc::clone(&env.client),
        Arc::clone(&env.pool),
        Arc::clone(&env.regs),
        Arc::clone(&env.tracker),
        tool_call_event(env.chat_id, "stub:echo", r#"{"input":"hello"}"#, "call_1"),
    )
    .await;
    assert_eq!(tool_messages(&env.client, env.chat_id).await.len(), 1);
}

#[tokio::test]
async fn mcp_error_becomes_tool_content() {
    let env = setup_registered_chat(None, vec![]).await;
    tool_handler::handle_tool_calls(
        Arc::clone(&env.client),
        Arc::clone(&env.pool),
        Arc::clone(&env.regs),
        Arc::clone(&env.tracker),
        tool_call_event(env.chat_id, "stub:fail", "{}", "call_err"),
    )
    .await;

    let results = tool_messages(&env.client, env.chat_id).await;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].tool_call_id.as_deref(), Some("call_err"));
    assert!(results[0].content.contains("MCP tool call failed"));
}

#[tokio::test]
async fn failed_call_marks_server_error_and_recovery_flips_it_back() {
    let env = setup_registered_chat(None, vec![]).await;

    tool_handler::handle_tool_calls(
        Arc::clone(&env.client),
        Arc::clone(&env.pool),
        Arc::clone(&env.regs),
        Arc::clone(&env.tracker),
        tool_call_event(env.chat_id, "stub:fail", "{}", "call_flip_err"),
    )
    .await;
    let payload: serde_json::Value =
        serde_json::from_str(&env.tracker.payload_json().await).unwrap();
    assert_eq!(payload["mcp"][0]["id"], "stub");
    assert_eq!(payload["mcp"][0]["status"], "error");
    assert!(payload["mcp"][0]["error"]
        .as_str()
        .unwrap()
        .contains("tool call failed:"));

    // A later successful call recovers the server.
    tool_handler::handle_tool_calls(
        Arc::clone(&env.client),
        Arc::clone(&env.pool),
        Arc::clone(&env.regs),
        Arc::clone(&env.tracker),
        tool_call_event(env.chat_id, "stub:echo", r#"{"input":"x"}"#, "call_flip_ok"),
    )
    .await;
    let payload: serde_json::Value =
        serde_json::from_str(&env.tracker.payload_json().await).unwrap();
    assert_eq!(payload["mcp"][0]["status"], "ok");
    assert!(payload["mcp"][0].get("error").is_none());
}

#[tokio::test]
async fn unregistered_server_and_foreign_tools_are_skipped() {
    let env = setup_registered_chat(None, vec![]).await;

    // Routed to our pool but server not registered for this chat (fresh regs).
    tool_handler::handle_tool_calls(
        Arc::clone(&env.client),
        Arc::clone(&env.pool),
        new_regs(),
        Arc::clone(&env.tracker),
        tool_call_event(env.chat_id, "stub:echo", r#"{"input":"x"}"#, "call_skip"),
    )
    .await;

    // Tool name we do not own at all.
    tool_handler::handle_tool_calls(
        Arc::clone(&env.client),
        Arc::clone(&env.pool),
        Arc::clone(&env.regs),
        Arc::clone(&env.tracker),
        tool_call_event(env.chat_id, "other:tool", "{}", "call_foreign"),
    )
    .await;

    assert!(tool_messages(&env.client, env.chat_id).await.is_empty());
}
