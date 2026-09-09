//! Unit tests for the `mcpStatus:1` tracker (pure parts — no server needed).

use rhd_plugin_mcp::mcp_pool::ServerStartupReport;
use rhd_plugin_mcp::status::McpStatusTracker;

fn report(id: &str, error: Option<&str>) -> ServerStartupReport {
    ServerStartupReport {
        id: id.to_string(),
        name: id.to_string(),
        error: error.map(|s| s.to_string()),
    }
}

#[tokio::test]
async fn payload_shape_matches_schema() {
    let tracker = McpStatusTracker::new(vec![
        ("good".to_string(), "good".to_string()),
        ("bad".to_string(), "bad".to_string()),
    ]);
    tracker
        .record_startup(&[
            report("good", None),
            report("bad", Some("spawn/initialize failed: nope")),
        ])
        .await;

    let payload: serde_json::Value =
        serde_json::from_str(&tracker.payload_json().await).unwrap();
    let mcp = payload["mcp"].as_array().unwrap();
    assert_eq!(mcp.len(), 2);

    // Config order is preserved; ok entries omit `error`.
    assert_eq!(mcp[0]["id"], "good");
    assert_eq!(mcp[0]["name"], "good");
    assert_eq!(mcp[0]["status"], "ok");
    assert!(mcp[0].get("error").is_none());

    assert_eq!(mcp[1]["id"], "bad");
    assert_eq!(mcp[1]["name"], "bad");
    assert_eq!(mcp[1]["status"], "error");
    assert_eq!(mcp[1]["error"], "spawn/initialize failed: nope");
}

#[tokio::test]
async fn mark_error_and_mark_ok_flip_status() {
    let tracker = McpStatusTracker::new(vec![("s".to_string(), "s".to_string())]);
    tracker.record_startup(&[report("s", None)]).await;

    let before: serde_json::Value =
        serde_json::from_str(&tracker.payload_json().await).unwrap();
    assert_eq!(before["mcp"][0]["status"], "ok");

    tracker.mark_error("s", "tool call failed: boom".to_string()).await;
    let after: serde_json::Value =
        serde_json::from_str(&tracker.payload_json().await).unwrap();
    assert_eq!(after["mcp"][0]["status"], "error");
    assert_eq!(after["mcp"][0]["error"], "tool call failed: boom");

    tracker.mark_ok("s").await;
    let recovered: serde_json::Value =
        serde_json::from_str(&tracker.payload_json().await).unwrap();
    assert_eq!(recovered["mcp"][0]["status"], "ok");
    assert!(recovered["mcp"][0].get("error").is_none());
}

#[tokio::test]
async fn record_startup_overrides_initial_ok_guess() {
    // `new` seeds every server as ok; a failed startup report must correct it.
    let tracker = McpStatusTracker::new(vec![("dead".to_string(), "dead".to_string())]);
    tracker
        .record_startup(&[report("dead", Some("spawn/initialize failed: gone"))])
        .await;

    let payload: serde_json::Value =
        serde_json::from_str(&tracker.payload_json().await).unwrap();
    assert_eq!(payload["mcp"][0]["status"], "error");
    assert_eq!(payload["mcp"][0]["error"], "spawn/initialize failed: gone");
}
