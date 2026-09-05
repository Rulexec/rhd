//! Integration tests for the MCP pool: startup, tool listing, routing, calls.

mod common;

use common::config_with;
use rhd_plugin_mcp::mcp_pool::McpPool;

#[tokio::test]
async fn pool_startup_lists_and_routes_stub_tools() {
    let pool = McpPool::startup(&config_with("stub", None)).await.unwrap();

    let mut names = pool.all_tool_names();
    names.sort();
    assert_eq!(names, vec!["stub:echo", "stub:fail"]);

    let route = pool.route("stub:echo").unwrap();
    assert_eq!(route.server_id, "stub");
    assert_eq!(route.tool_name, "echo");

    let result = pool
        .call_tool("stub", "echo", r#"{"input":"hi"}"#)
        .await
        .unwrap();
    assert_eq!(result.content, "echo: hi");
    assert_ne!(result.is_error, Some(true));

    assert!(pool.call_tool("stub", "fail", "{}").await.is_err());
}

#[tokio::test]
async fn pool_startup_fails_fast_on_bad_cmd() {
    let mut cfg = config_with("bad", None);
    cfg.servers[0].cmd = "/nonexistent/mcp-binary".to_string();
    assert!(McpPool::startup(&cfg).await.is_err());
}
