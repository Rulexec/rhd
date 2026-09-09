//! Integration tests for the MCP pool: startup, tool listing, routing, calls.

mod common;

use common::config_with;
use rhd_plugin_mcp::mcp_pool::McpPool;

#[tokio::test]
async fn pool_startup_lists_and_routes_stub_tools() {
    let (pool, reports) = McpPool::startup(&config_with("stub", None)).await;
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].error, None);

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
async fn pool_startup_reports_bad_cmd_without_failing() {
    let mut cfg = config_with("bad", None);
    cfg.servers[0].cmd = "/nonexistent/mcp-binary".to_string();
    let (pool, reports) = McpPool::startup(&cfg).await;

    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, "bad");
    let error = reports[0].error.as_deref().unwrap();
    assert!(
        error.starts_with("spawn/initialize failed:"),
        "unexpected startup error: {error}"
    );

    // The failed server is excluded from routing but still listed in config order.
    assert!(pool.route("bad:echo").is_none());
    assert!(pool.all_tool_names().is_empty());
    assert_eq!(pool.server_ids(), vec![("bad".to_string(), "bad".to_string())]);
}
