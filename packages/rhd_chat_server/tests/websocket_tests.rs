//! Integration tests for WebSocket protocol using rhd_chat_client.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::{Mutex, Notify};
use rhd_db::ChatDb;

use rhd_chat_api::{
    AckCustomEventParams, AddMessageParams, CreateChatParams, GetChatParams, GetPluginsParams,
    ListChatsParams, RegisterPluginParams, SendCustomEventParams, SubscribeChatParams,
};
use rhd_chat_client::{ChatClient, ChatEvent};
use rhd_chat_server::config::Config;
use rhd_chat_server::connection::handle_connection;
use rhd_chat_server::plugins::new_shared_plugin_registry;
use rhd_chat_server::subscriptions::new_shared_subscription_manager;

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

async fn connect_client(port: u16) -> ChatClient {
    let url = format!("ws://127.0.0.1:{}/", port);
    ChatClient::connect(&url).await.unwrap()
}

#[tokio::test]
async fn test_create_chat() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;

    let result = client
        .create_chat(CreateChatParams {
            title: "Test Chat".to_string(),
            tags: vec!["test".to_string()],
        })
        .await
        .unwrap();

    assert!(result.chat_id > 0);
}

#[tokio::test]
async fn test_list_chats() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;

    // Create a chat first
    client
        .create_chat(CreateChatParams {
            title: "Test".to_string(),
            tags: vec![],
        })
        .await
        .unwrap();

    // List chats
    let result = client.list_chats(ListChatsParams { tags: vec![] }).await.unwrap();

    assert!(!result.chats.is_empty());
}

#[tokio::test]
async fn test_get_chat() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;

    // Create a chat
    let create_result = client
        .create_chat(CreateChatParams {
            title: "Test".to_string(),
            tags: vec![],
        })
        .await
        .unwrap();
    let chat_id = create_result.chat_id;

    // Get chat
    let result = client
        .get_chat(GetChatParams { chat_id })
        .await
        .unwrap();

    assert_eq!(result.chat.id, chat_id);
}

#[tokio::test]
async fn test_add_message() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;

    // Create a chat
    let create_result = client
        .create_chat(CreateChatParams {
            title: "Test".to_string(),
            tags: vec![],
        })
        .await
        .unwrap();
    let chat_id = create_result.chat_id;

    // Add message
    let result = client
        .add_message(AddMessageParams {
            chat_id,
            role: "user".to_string(),
            content: "Hello".to_string(),
            reasoning_content: None,
            tags: vec!["greeting".to_string()],
        })
        .await
        .unwrap();

    assert!(result.message_id > 0);
}

#[tokio::test]
async fn test_subscription_chat_events() {
    let (port, _handle) = start_test_server().await;

    // Connection 1: subscribe to chat
    let client1 = connect_client(port).await;
    let create_result = client1
        .create_chat(CreateChatParams {
            title: "Test".to_string(),
            tags: vec![],
        })
        .await
        .unwrap();
    let chat_id = create_result.chat_id;

    client1
        .subscribe_chat(SubscribeChatParams { chat_id })
        .await
        .unwrap();

    // Set up event notification
    let event_received = Arc::new(Notify::new());
    let event_received_clone = event_received.clone();

    let _cancel_token = client1.on_chat_event(chat_id, move |event| {
        let event_received = event_received_clone.clone();
        async move {
            match event {
                ChatEvent::MessageAdded(data) => {
                    assert_eq!(data.chat_id, chat_id);
                    assert_eq!(data.message.content, "Test message");
                    event_received.notify_one();
                }
                _ => {}
            }
        }
    });

    // Connection 2: add message
    let client2 = connect_client(port).await;
    client2
        .add_message(AddMessageParams {
            chat_id,
            role: "user".to_string(),
            content: "Test message".to_string(),
            reasoning_content: None,
            tags: vec![],
        })
        .await
        .unwrap();

    // Connection 1 should receive messageAdded event
    let result = tokio::time::timeout(Duration::from_secs(5), event_received.notified()).await;
    assert!(result.is_ok(), "Did not receive messageAdded event");
}

#[tokio::test]
async fn test_plugin_registration() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;

    let result = client
        .register_plugin(RegisterPluginParams {
            plugin_id: "test-plugin".to_string(),
        })
        .await
        .unwrap();

    // RegisterPluginResult is empty, so just check that the call succeeded
    let _ = result;

    // Get plugins
    let plugins_result = client.get_plugins(GetPluginsParams {}).await.unwrap();
    assert!(plugins_result
        .plugins
        .iter()
        .any(|p| p.plugin_id == "test-plugin"));
}

#[tokio::test]
async fn test_custom_event_flow() {
    let (port, _handle) = start_test_server().await;

    // Connect both clients first
    let client1 = connect_client(port).await;
    let client2 = connect_client(port).await;

    // Register plugins on both connections
    client1
        .register_plugin(RegisterPluginParams {
            plugin_id: "sender".to_string(),
        })
        .await
        .unwrap();
    client2
        .register_plugin(RegisterPluginParams {
            plugin_id: "receiver".to_string(),
        })
        .await
        .unwrap();

    // Set up event notification for client2
    let event_received = Arc::new(Notify::new());
    let event_received_clone = event_received.clone();
    let received_event_id = Arc::new(Mutex::new(String::new()));
    let received_event_id_clone = received_event_id.clone();

    let _cancel_token = client2.on_custom_event(move |event| {
        let event_received = event_received_clone.clone();
        let received_event_id = received_event_id_clone.clone();
        async move {
            *received_event_id.lock().await = event.event_id.clone();
            event_received.notify_one();
        }
    });

    // Client1 sends custom event (both clients are connected)
    let send_result = client1
        .send_custom_event(SendCustomEventParams {
            event_name: "test-event".to_string(),
            additional: Some("{\"key\": \"value\"}".to_string()),
        })
        .await
        .unwrap();
    let event_id = send_result.event_id.clone();
    let event_id_for_ack = send_result.event_id;

    // Client2 should receive customEvent
    let result = tokio::time::timeout(Duration::from_secs(5), event_received.notified()).await;
    assert!(result.is_ok(), "Did not receive customEvent");
    assert_eq!(*received_event_id.lock().await, event_id);

    // Set up acknowledgment notification for client1
    let ack_received = Arc::new(Notify::new());
    let ack_received_clone = ack_received.clone();

    let _cancel_token = client1.on_custom_event_acknowledged(move |event| {
        let ack_received = ack_received_clone.clone();
        let expected_event_id = event_id.clone();
        async move {
            assert_eq!(event.event_id, expected_event_id);
            ack_received.notify_one();
        }
    });

    // Client2: acknowledge event
    client2
        .ack_custom_event(AckCustomEventParams {
            event_id: event_id_for_ack,
        })
        .await
        .unwrap();

    // Client1 should receive customEventAcknowledged
    let result = tokio::time::timeout(Duration::from_secs(5), ack_received.notified()).await;
    assert!(result.is_ok(), "Did not receive customEventAcknowledged");
}
