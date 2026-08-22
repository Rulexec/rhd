//! Integration tests for WebSocket protocol.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream, MaybeTlsStream};
use tokio::net::TcpStream;
use rhd_db::ChatDb;
use uuid::Uuid;

use rhd_chat_server::config::Config;
use rhd_chat_server::plugins::new_shared_plugin_registry;
use rhd_chat_server::subscriptions::new_shared_subscription_manager;
use rhd_chat_server::connection::handle_connection;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

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
    };

    // Initialize database
    let db = Arc::new(ChatDb::new(&config.db_path).unwrap());

    // Create subscription manager
    let subscription_manager = new_shared_subscription_manager();

    // Create plugin registry
    let plugin_registry = new_shared_plugin_registry();

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

            tokio::spawn(async move {
                match tokio_tungstenite::accept_async(stream).await {
                    Ok(ws_stream) => {
                        let (write, read) = ws_stream.split();
                        let _ = handle_connection(read, write, db, subscription_manager, plugin_registry).await;
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

async fn connect_to_server(port: u16) -> WsStream {
    let url = format!("ws://127.0.0.1:{}/", port);
    let (ws_stream, _) = connect_async(&url).await.unwrap();
    ws_stream
}

async fn send_request(ws: &mut WsStream, method: &str, params: Value) -> Value {
    let request_id = Uuid::new_v4().to_string();
    let request = json!({
        "type": "request",
        "id": request_id,
        "method": method,
        "params": params
    });
    ws.send(Message::Text(serde_json::to_string(&request).unwrap())).await.unwrap();
    
    // Wait for response with matching ID
    loop {
        let msg = timeout(Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap();
        match msg {
            Message::Text(text) => {
                let response: Value = serde_json::from_str(&text).unwrap();
                // Check if this is a response with our request ID
                if response.get("type") == Some(&json!("response"))
                   && response.get("id") == Some(&json!(request_id)) {
                    return response;
                }
                // Otherwise, continue reading messages
            }
            _ => panic!("Expected text message"),
        }
    }
}

#[tokio::test]
async fn test_create_chat() {
    let (port, _handle) = start_test_server().await;
    let mut ws = connect_to_server(port).await;
    
    let response = send_request(&mut ws, "createChat", json!({
        "title": "Test Chat",
        "tags": ["test"]
    })).await;
    
    assert_eq!(response["success"], true);
    assert!(response["data"]["chatId"].is_number());
}

#[tokio::test]
async fn test_list_chats() {
    let (port, _handle) = start_test_server().await;
    let mut ws = connect_to_server(port).await;
    
    // Create a chat first
    send_request(&mut ws, "createChat", json!({"title": "Test"})).await;
    
    // List chats
    let response = send_request(&mut ws, "listChats", json!({})).await;
    
    assert_eq!(response["success"], true);
    assert!(response["data"]["chats"].is_array());
    assert!(response["data"]["chats"].as_array().unwrap().len() > 0);
}

#[tokio::test]
async fn test_get_chat() {
    let (port, _handle) = start_test_server().await;
    let mut ws = connect_to_server(port).await;
    
    // Create a chat
    let create_response = send_request(&mut ws, "createChat", json!({"title": "Test"})).await;
    let chat_id = create_response["data"]["chatId"].as_i64().unwrap();
    
    // Get chat
    let response = send_request(&mut ws, "getChat", json!({"chatId": chat_id})).await;
    
    assert_eq!(response["success"], true);
    assert_eq!(response["data"]["chat"]["id"], chat_id);
}

#[tokio::test]
async fn test_add_message() {
    let (port, _handle) = start_test_server().await;
    let mut ws = connect_to_server(port).await;
    
    // Create a chat
    let create_response = send_request(&mut ws, "createChat", json!({"title": "Test"})).await;
    let chat_id = create_response["data"]["chatId"].as_i64().unwrap();
    
    // Add message
    let response = send_request(&mut ws, "addMessage", json!({
        "chatId": chat_id,
        "role": "user",
        "content": "Hello",
        "tags": ["greeting"]
    })).await;
    
    assert_eq!(response["success"], true);
    assert!(response["data"]["messageId"].is_number());
}

#[tokio::test]
async fn test_subscription_chat_events() {
    let (port, _handle) = start_test_server().await;
    
    // Connection 1: subscribe to chat
    let mut ws1 = connect_to_server(port).await;
    let create_response = send_request(&mut ws1, "createChat", json!({"title": "Test"})).await;
    let chat_id = create_response["data"]["chatId"].as_i64().unwrap();
    
    send_request(&mut ws1, "subscribeChat", json!({"chatId": chat_id})).await;
    
    // Connection 2: add message
    let mut ws2 = connect_to_server(port).await;
    send_request(&mut ws2, "addMessage", json!({
        "chatId": chat_id,
        "role": "user",
        "content": "Test message"
    })).await;
    
    // Connection 1 should receive messageAdded event
    let msg = timeout(Duration::from_secs(5), ws1.next()).await.unwrap().unwrap().unwrap();
    match msg {
        Message::Text(text) => {
            let event: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(event["type"], "event");
            assert_eq!(event["event"], "messageAdded");
        }
        _ => panic!("Expected text message"),
    }
}

#[tokio::test]
async fn test_plugin_registration() {
    let (port, _handle) = start_test_server().await;
    let mut ws = connect_to_server(port).await;
    
    let response = send_request(&mut ws, "registerPlugin", json!({
        "pluginId": "test-plugin"
    })).await;
    
    assert_eq!(response["success"], true);
    
    // Get plugins
    let response = send_request(&mut ws, "getPlugins", json!({})).await;
    assert_eq!(response["success"], true);
    let plugins = response["data"]["plugins"].as_array().unwrap();
    assert!(plugins.iter().any(|p| p["pluginId"] == "test-plugin"));
}

#[tokio::test]
async fn test_custom_event_flow() {
    let (port, _handle) = start_test_server().await;
    
    // Connect both clients first
    let mut ws1 = connect_to_server(port).await;
    let mut ws2 = connect_to_server(port).await;
    
    // Register plugins on both connections
    send_request(&mut ws1, "registerPlugin", json!({"pluginId": "sender"})).await;
    send_request(&mut ws2, "registerPlugin", json!({"pluginId": "receiver"})).await;
    
    // Now ws1 sends custom event (both clients are connected)
    let send_response = send_request(&mut ws1, "sendCustomEvent", json!({
        "eventName": "test-event",
        "additional": "{\"key\": \"value\"}"
    })).await;
    let event_id = send_response["data"]["eventId"].as_str().unwrap().to_string();
    
    // Connection 2 should receive customEvent
    // First, read messages until we get the customEvent (skip any pluginRegistered events)
    let mut received_custom_event = false;
    for _ in 0..5 {
        let msg = timeout(Duration::from_secs(5), ws2.next()).await.unwrap().unwrap().unwrap();
        match msg {
            Message::Text(text) => {
                let event: Value = serde_json::from_str(&text).unwrap();
                if event["type"] == "event" && event["event"] == "customEvent" {
                    assert_eq!(event["data"]["eventId"], event_id);
                    received_custom_event = true;
                    break;
                }
            }
            _ => panic!("Expected text message"),
        }
    }
    assert!(received_custom_event, "Did not receive customEvent");
    
    // Connection 2: acknowledge event
    send_request(&mut ws2, "ackCustomEvent", json!({"eventId": event_id})).await;
    
    // Connection 1 should receive customEventAcknowledged
    let msg = timeout(Duration::from_secs(5), ws1.next()).await.unwrap().unwrap().unwrap();
    match msg {
        Message::Text(text) => {
            let event: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(event["type"], "event");
            assert_eq!(event["event"], "customEventAcknowledged");
            assert_eq!(event["data"]["eventId"], event_id);
        }
        _ => panic!("Expected text message"),
    }
}
