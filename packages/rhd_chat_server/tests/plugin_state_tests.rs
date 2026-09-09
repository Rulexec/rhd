//! Integration tests for plugin state methods over WebSocket.
//!
//! Uses raw `Request`/`Response` JSON frames via tokio_tungstenite because the
//! typed client methods land in Phase 4 (`rhd_chat_client`).

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use rhd_chat_api::protocol::Request;
use rhd_db::ChatDb;

use rhd_chat_server::config::Config;
use rhd_chat_server::connection::handle_connection;
use rhd_chat_server::plugins::new_shared_plugin_registry;
use rhd_chat_server::streams::StreamManager;
use rhd_chat_server::subscriptions::new_shared_subscription_manager;
use std::sync::Arc;

async fn start_test_server() -> (u16, tokio::task::JoinHandle<()>) {
    // Find available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Create test config with in-memory database
    let config = Config {
        host: "127.0.0.1".to_string(),
        port,
        db_path: ":memory:".to_string(),
        clear_pending_acks: false,
    };

    // Initialize database
    let db = Arc::new(ChatDb::new(&config.db_path).unwrap());

    // Create subscription manager
    let subscription_manager = new_shared_subscription_manager();

    // Create plugin registry
    let plugin_registry = new_shared_plugin_registry();

    // Create stream manager
    let stream_manager = Arc::new(StreamManager::new());

    // Bind TCP listener
    let listener = tokio::net::TcpListener::bind(&config.socket_addr()).await.unwrap();

    // Start server in background
    let handle = tokio::spawn(async move {
        loop {
            let (stream, _addr) = match listener.accept().await {
                Ok(result) => result,
                Err(_) => break,
            };

            let db = Arc::clone(&db);
            let subscription_manager = subscription_manager.clone();
            let plugin_registry = plugin_registry.clone();
            let stream_manager = stream_manager.clone();

            tokio::spawn(async move {
                match tokio_tungstenite::accept_async(stream).await {
                    Ok(ws_stream) => {
                        let (write, read) = ws_stream.split();
                        let _ = handle_connection(read, write, db, subscription_manager, plugin_registry, stream_manager).await;
                    }
                    Err(_) => {}
                }
            });
        }
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    (port, handle)
}

/// Minimal raw WebSocket client for driving the plugin-state protocol without
/// the typed client (Phase 4). Buffers events that arrive while waiting for a
/// response so they can be asserted later.
struct RawClient {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    next_id: u64,
    buffered_events: Vec<Value>,
}

impl RawClient {
    async fn connect(port: u16) -> Self {
        let url = format!("ws://127.0.0.1:{}/", port);
        let (ws, _) = connect_async(&url).await.unwrap();
        RawClient {
            ws,
            next_id: 0,
            buffered_events: Vec::new(),
        }
    }

    /// Send a request and return the matching response frame, buffering any
    /// events that arrive in between.
    async fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = format!("req-{}", self.next_id);
        let req = Request::new(&id, method, params);
        self.ws
            .send(Message::Text(serde_json::to_string(&req).unwrap()))
            .await
            .unwrap();

        loop {
            let frame = self.next_frame().await;
            if frame["type"] == "response" && frame["id"] == id {
                return frame;
            }
            if frame["type"] == "event" {
                self.buffered_events.push(frame);
            }
        }
    }

    /// Return the next event frame with the given name, checking the buffer
    /// first and then reading frames until it arrives (with a timeout).
    async fn wait_for_event(&mut self, name: &str) -> Value {
        if let Some(pos) = self
            .buffered_events
            .iter()
            .position(|e| e["event"] == Value::String(name.to_string()))
        {
            return self.buffered_events.remove(pos);
        }
        loop {
            let frame = self.next_frame().await;
            if frame["type"] == "event" {
                if frame["event"] == Value::String(name.to_string()) {
                    return frame;
                }
                self.buffered_events.push(frame);
            }
        }
    }

    async fn next_frame(&mut self) -> Value {
        loop {
            let msg = tokio::time::timeout(Duration::from_secs(5), self.ws.next())
                .await
                .expect("timed out waiting for a frame")
                .expect("websocket stream ended")
                .expect("websocket error");
            match msg {
                Message::Text(text) => {
                    return serde_json::from_str(&text).unwrap();
                }
                Message::Close(_) => panic!("connection closed by server"),
                _ => continue,
            }
        }
    }

    /// Close the connection so the server runs its disconnect cleanup.
    async fn shutdown(mut self) {
        let _ = self.ws.close(None).await;
    }
}

fn update_params(key: &str, content: &str, format: &str, schema: &str) -> Value {
    json!({ "key": key, "content": content, "format": format, "schema": schema })
}

async fn register_plugin(client: &mut RawClient, plugin_id: &str) {
    let resp = client
        .request("registerPlugin", json!({ "pluginId": plugin_id }))
        .await;
    assert_eq!(resp["success"], json!(true), "registerPlugin failed: {resp}");
}

#[tokio::test]
async fn test_update_requires_registered_plugin() {
    let (port, _handle) = start_test_server().await;
    let mut client = RawClient::connect(port).await;

    let resp = client
        .request(
            "updatePluginState",
            update_params("status", "hello", "markdown", ""),
        )
        .await;

    assert_eq!(resp["success"], json!(false));
    assert_eq!(resp["errorCode"], json!("INVALID_REQUEST"));
    assert_eq!(resp["error"], json!("Connection has no registered plugin"));
}

#[tokio::test]
async fn test_upsert_versioning_flow() {
    let (port, _handle) = start_test_server().await;
    let mut client = RawClient::connect(port).await;
    register_plugin(&mut client, "ps-test").await;

    let resp = client
        .request(
            "updatePluginState",
            update_params("status", "v1 content", "json", "mcpStatus:1"),
        )
        .await;
    assert_eq!(resp["success"], json!(true), "update failed: {resp}");
    assert_eq!(resp["data"]["state"]["version"], json!(1));
    assert_eq!(resp["data"]["state"]["pluginId"], json!("ps-test"));
    assert_eq!(resp["data"]["state"]["key"], json!("status"));

    let resp = client
        .request(
            "updatePluginState",
            update_params("status", "v2 content", "json", "mcpStatus:1"),
        )
        .await;
    assert_eq!(resp["success"], json!(true), "update failed: {resp}");
    assert_eq!(resp["data"]["state"]["version"], json!(2));

    let resp = client.request("getPluginStates", json!({})).await;
    assert_eq!(resp["success"], json!(true));
    let states = resp["data"]["states"].as_array().unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0]["version"], json!(2));
    assert_eq!(states[0]["content"], json!("v2 content"));
    assert_eq!(states[0]["format"], json!("json"));
    assert_eq!(states[0]["schema"], json!("mcpStatus:1"));
    assert!(states[0]["updatedAt"].is_string());
}

#[tokio::test]
async fn test_remove_bumps_version_and_hides() {
    let (port, _handle) = start_test_server().await;
    let mut plugin_conn = RawClient::connect(port).await;
    register_plugin(&mut plugin_conn, "ps-rm").await;

    for content in ["v1", "v2"] {
        let resp = plugin_conn
            .request(
                "updatePluginState",
                update_params("status", content, "json", "s:1"),
            )
            .await;
        assert_eq!(resp["success"], json!(true));
    }

    let mut subscriber = RawClient::connect(port).await;
    let resp = subscriber.request("subscribePluginStates", json!({ "states": [] })).await;
    assert_eq!(resp["success"], json!(true));

    let resp = plugin_conn
        .request("removePluginState", json!({ "key": "status" }))
        .await;
    assert_eq!(resp["success"], json!(true), "remove failed: {resp}");

    let event = subscriber.wait_for_event("pluginStateRemoved").await;
    assert_eq!(event["data"]["pluginId"], json!("ps-rm"));
    assert_eq!(event["data"]["key"], json!("status"));
    assert_eq!(event["data"]["version"], json!(3));

    let resp = plugin_conn.request("getPluginStates", json!({})).await;
    assert_eq!(resp["data"]["states"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_recreate_after_remove_is_monotonic() {
    let (port, _handle) = start_test_server().await;
    let mut client = RawClient::connect(port).await;
    register_plugin(&mut client, "ps-mono").await;

    let resp = client
        .request(
            "updatePluginState",
            update_params("status", "first", "json", "s:1"),
        )
        .await;
    assert_eq!(resp["data"]["state"]["version"], json!(1));

    let resp = client.request("removePluginState", json!({ "key": "status" })).await;
    assert_eq!(resp["success"], json!(true));

    let resp = client
        .request(
            "updatePluginState",
            update_params("status", "recreated", "json", "s:1"),
        )
        .await;
    assert_eq!(resp["success"], json!(true));
    assert_eq!(resp["data"]["state"]["version"], json!(3));

    let resp = client.request("getPluginStates", json!({})).await;
    let states = resp["data"]["states"].as_array().unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0]["version"], json!(3));
    assert_eq!(states[0]["content"], json!("recreated"));
}

#[tokio::test]
async fn test_subscribe_receives_broadcasts() {
    let (port, _handle) = start_test_server().await;
    let mut plugin_conn = RawClient::connect(port).await;
    register_plugin(&mut plugin_conn, "ps-bcast").await;

    let mut subscriber_a = RawClient::connect(port).await;
    let mut subscriber_b = RawClient::connect(port).await;
    for subscriber in [&mut subscriber_a, &mut subscriber_b] {
        let resp = subscriber
            .request("subscribePluginStates", json!({ "states": [] }))
            .await;
        assert_eq!(resp["success"], json!(true));
    }

    let resp = plugin_conn
        .request(
            "updatePluginState",
            update_params("status", "{\"mcp\":[]}", "json", "mcpStatus:1"),
        )
        .await;
    assert_eq!(resp["success"], json!(true));

    for subscriber in [&mut subscriber_a, &mut subscriber_b] {
        let event = subscriber.wait_for_event("pluginStateChanged").await;
        assert_eq!(event["data"]["state"]["pluginId"], json!("ps-bcast"));
        assert_eq!(event["data"]["state"]["key"], json!("status"));
        assert_eq!(event["data"]["state"]["version"], json!(1));
        assert_eq!(event["data"]["state"]["content"], json!("{\"mcp\":[]}"));
        assert_eq!(event["data"]["state"]["format"], json!("json"));
        assert_eq!(event["data"]["state"]["schema"], json!("mcpStatus:1"));
    }
}

#[tokio::test]
async fn test_subscribe_catchup_returns_newer_only() {
    let (port, _handle) = start_test_server().await;
    let mut plugin_conn = RawClient::connect(port).await;
    register_plugin(&mut plugin_conn, "ps-catch").await;

    for content in ["v1", "v2"] {
        let resp = plugin_conn
            .request(
                "updatePluginState",
                update_params("status", content, "json", "s:1"),
            )
            .await;
        assert_eq!(resp["success"], json!(true));
    }

    // Stale version 1 → catch-up returns the current v2 state.
    let mut stale = RawClient::connect(port).await;
    let resp = stale
        .request(
            "subscribePluginStates",
            json!({ "states": [{ "pluginId": "ps-catch", "key": "status", "version": 1 }] }),
        )
        .await;
    assert_eq!(resp["success"], json!(true));
    let states = resp["data"]["states"].as_array().unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0]["version"], json!(2));
    assert_eq!(states[0]["content"], json!("v2"));

    // Current version 2 → nothing newer, empty catch-up.
    let mut current = RawClient::connect(port).await;
    let resp = current
        .request(
            "subscribePluginStates",
            json!({ "states": [{ "pluginId": "ps-catch", "key": "status", "version": 2 }] }),
        )
        .await;
    assert_eq!(resp["success"], json!(true));
    assert_eq!(resp["data"]["states"].as_array().unwrap().len(), 0);

    // Version 0 → always return the latest if present.
    let mut fresh = RawClient::connect(port).await;
    let resp = fresh
        .request(
            "subscribePluginStates",
            json!({ "states": [{ "pluginId": "ps-catch", "key": "status", "version": 0 }] }),
        )
        .await;
    assert_eq!(resp["success"], json!(true));
    let states = resp["data"]["states"].as_array().unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0]["version"], json!(2));
}

#[tokio::test]
async fn test_get_plugin_states_filters() {
    let (port, _handle) = start_test_server().await;
    let mut conn_a = RawClient::connect(port).await;
    register_plugin(&mut conn_a, "ps-f1").await;
    conn_a
        .request(
            "updatePluginState",
            update_params("status", "a-status", "json", "mcpStatus:1"),
        )
        .await;
    conn_a
        .request(
            "updatePluginState",
            update_params("notes", "a-notes", "markdown", "notes:1"),
        )
        .await;

    let mut conn_b = RawClient::connect(port).await;
    register_plugin(&mut conn_b, "ps-f2").await;
    conn_b
        .request(
            "updatePluginState",
            update_params("status", "b-status", "json", "mcpStatus:1"),
        )
        .await;

    let resp = conn_a.request("getPluginStates", json!({})).await;
    assert_eq!(resp["data"]["states"].as_array().unwrap().len(), 3);

    let resp = conn_a
        .request("getPluginStates", json!({ "pluginId": "ps-f1" }))
        .await;
    let states = resp["data"]["states"].as_array().unwrap();
    assert_eq!(states.len(), 2);
    assert!(states.iter().all(|s| s["pluginId"] == json!("ps-f1")));

    let resp = conn_a
        .request("getPluginStates", json!({ "schema": "mcpStatus:1" }))
        .await;
    let states = resp["data"]["states"].as_array().unwrap();
    assert_eq!(states.len(), 2);
    assert!(states.iter().all(|s| s["schema"] == json!("mcpStatus:1")));

    let resp = conn_a
        .request(
            "getPluginStates",
            json!({ "pluginId": "ps-f2", "schema": "notes:1" }),
        )
        .await;
    assert_eq!(resp["data"]["states"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_remove_plugin_cascades_states() {
    let (port, _handle) = start_test_server().await;
    let mut client = RawClient::connect(port).await;
    register_plugin(&mut client, "ps-cascade").await;

    let resp = client
        .request(
            "updatePluginState",
            update_params("status", "hello", "json", "s:1"),
        )
        .await;
    assert_eq!(resp["success"], json!(true));

    let resp = client
        .request("removePlugin", json!({ "pluginId": "ps-cascade" }))
        .await;
    assert_eq!(resp["success"], json!(true), "removePlugin failed: {resp}");

    let resp = client.request("getPluginStates", json!({})).await;
    assert_eq!(resp["data"]["states"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_states_survive_disconnect() {
    let (port, _handle) = start_test_server().await;
    let mut plugin_conn = RawClient::connect(port).await;
    register_plugin(&mut plugin_conn, "ps-survive").await;

    let resp = plugin_conn
        .request(
            "updatePluginState",
            update_params("status", "last known", "json", "s:1"),
        )
        .await;
    assert_eq!(resp["success"], json!(true));

    plugin_conn.shutdown().await;

    // Wait for the server to process the disconnect (plugin marked inactive),
    // then verify the state row is still returned.
    let mut reader = RawClient::connect(port).await;
    let mut deactivated = false;
    for _ in 0..50 {
        let resp = reader.request("getPlugins", json!({})).await;
        let plugins = resp["data"]["plugins"].as_array().unwrap();
        if let Some(entry) = plugins
            .iter()
            .find(|p| p["pluginId"] == json!("ps-survive"))
        {
            if entry["isActive"] == json!(false) {
                deactivated = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(deactivated, "plugin was never marked inactive");

    let resp = reader.request("getPluginStates", json!({})).await;
    let states = resp["data"]["states"].as_array().unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0]["pluginId"], json!("ps-survive"));
    assert_eq!(states[0]["version"], json!(1));
    assert_eq!(states[0]["content"], json!("last known"));
}
