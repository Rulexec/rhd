//! Integration tests for WebSocket protocol using rhd_chat_client.

use std::sync::Arc;
use std::sync::Once;

static INIT_TRACING: Once = Once::new();

fn init_tracing() {
    INIT_TRACING.call_once(|| {
        tracing_subscriber::fmt::init();
    });
}
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::{Mutex, Notify};
use rhd_db::ChatDb;

use rhd_chat_api::{
    AckCustomEventParams, AddMessageParams, CreateChatParams, GetChatParams, GetPluginsParams,
    ListChatsParams, RegisterPluginParams, SendCustomEventParams, SubscribeChatParams,
    UpdateMessageParams, UpdateToolCallTagsParams,
};
use rhd_chat_client::{ChatClient, ChatEvent, ClientError};
use rhd_chat_server::config::Config;
use rhd_chat_server::connection::handle_connection;
use rhd_chat_server::plugins::new_shared_plugin_registry;
use rhd_chat_server::streams::StreamManager;
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

async fn connect_client(port: u16) -> ChatClient {
    let url = format!("ws://127.0.0.1:{}/", port);
    ChatClient::connect(&url).await.unwrap()
}

#[tokio::test]
async fn test_create_chat() {
    init_tracing();
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
        .get_chat(GetChatParams { chat_id, if_version_higher_than: None })
        .await
        .unwrap();

    assert_eq!(result.chat.id, chat_id);
}

#[tokio::test]
async fn test_get_chat_includes_queue_count() {
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

    // Add queue messages
    client
        .add_queue_message(rhd_chat_api::AddQueueMessageParams {
            chat_id,
            role: "user".to_string(),
            content: "Queue message 1".to_string(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
        })
        .await
        .unwrap();

    client
        .add_queue_message(rhd_chat_api::AddQueueMessageParams {
            chat_id,
            role: "user".to_string(),
            content: "Queue message 2".to_string(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
        })
        .await
        .unwrap();

    // Get chat
    let result = client
        .get_chat(GetChatParams { chat_id, if_version_higher_than: None })
        .await
        .unwrap();

    // Verify queue count
    assert_eq!(result.queued_messages_count, 2);
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
            tool_call_id: None,
            reasoning_content: None,
            tags: vec!["greeting".to_string()],
            is_finished: true,
            is_streaming: false,
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
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .unwrap();

    // Connection 1 should receive messageAdded event
    let result = tokio::time::timeout(Duration::from_secs(5), event_received.notified()).await;
    assert!(result.is_ok(), "Did not receive messageAdded event");
}

#[tokio::test]
async fn test_update_tool_call_tags_broadcasts_message_updated() {
    init_tracing();
    let (port, _handle) = start_test_server().await;

    // Connection 1: subscribe to chat and capture messageUpdated events
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

    let latest_update = Arc::new(Mutex::new(None));
    let latest_update_clone = latest_update.clone();
    let event_received = Arc::new(Notify::new());
    let event_received_clone = event_received.clone();

    let _cancel_token = client1.on_chat_event(chat_id, move |event| {
        let latest_update = latest_update_clone.clone();
        let event_received = event_received_clone.clone();
        async move {
            if let ChatEvent::MessageUpdated(data) = event {
                *latest_update.lock().await = Some(data);
                event_received.notify_one();
            }
        }
    });

    // Connection 2: add assistant message, then set tool calls
    // (same JSON shape the ai_completions plugin writes)
    let client2 = connect_client(port).await;
    let add_result = client2
        .add_message(AddMessageParams {
            chat_id,
            role: "assistant".to_string(),
            content: "calling tool".to_string(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .unwrap();

    client2
        .update_message(UpdateMessageParams {
            message_id: add_result.message_id,
            content: None,
            reasoning_content: None,
            role: None,
            add_tags: vec![],
            remove_tags: vec![],
            is_finished: None,
            is_streaming: None,
            tool_calls: Some(
                r#"[{"id":"call_1","type":"function","function":{"name":"get_weather","arguments":"{}"}}]"#
                    .to_string(),
            ),
        })
        .await
        .unwrap();

    // Consume the messageUpdated event emitted by updateMessage
    tokio::time::timeout(Duration::from_secs(5), event_received.notified())
        .await
        .expect("Did not receive messageUpdated after setting tool calls");

    let version_before = client1
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .unwrap()
        .chat
        .version;

    // Add tags to the tool call
    client2
        .update_tool_call_tags(UpdateToolCallTagsParams {
            message_id: add_result.message_id,
            tool_call_id: "call_1".to_string(),
            add_tags: vec!["reviewed".to_string()],
            remove_tags: vec![],
        })
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_secs(5), event_received.notified())
        .await
        .expect("Did not receive messageUpdated after updateToolCallTags");

    let data = latest_update
        .lock()
        .await
        .take()
        .expect("messageUpdated event captured");
    assert_eq!(data.chat_id, chat_id);
    assert_eq!(
        data.chat_version,
        version_before + 1,
        "messageUpdated must carry the chat version covering the tag change"
    );
    assert_eq!(data.message.tool_calls.len(), 1);
    assert_eq!(data.message.tool_calls[0].id, "call_1");
    assert_eq!(data.message.tool_calls[0].call_type, "function");
    assert_eq!(
        data.message.tool_calls[0].tags,
        vec!["reviewed".to_string()]
    );

    // Unknown tool call id is rejected
    let err = client2
        .update_tool_call_tags(UpdateToolCallTagsParams {
            message_id: add_result.message_id,
            tool_call_id: "missing".to_string(),
            add_tags: vec!["x".to_string()],
            remove_tags: vec![],
        })
        .await
        .unwrap_err();
    assert!(matches!(err, ClientError::Server { .. }));
    assert!(err.to_string().contains("not found"));
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

/// Test: streamPush creates stream and delivers to stream subscribers.
#[tokio::test]
async fn test_stream_push_broadcasts() {
    let (port, _handle) = start_test_server().await;

    // Connect two clients
    let client1 = connect_client(port).await;
    let client2 = connect_client(port).await;

    // Create a chat
    let create_result = client1
        .create_chat(CreateChatParams {
            title: "Stream Test".to_string(),
            tags: vec![],
        })
        .await
        .unwrap();
    let chat_id = create_result.chat_id;

    // Client1 subscribes to chat, client2 subscribes to stream
    client1
        .subscribe_chat(SubscribeChatParams { chat_id })
        .await
        .unwrap();
    client2
        .stream_subscribe(rhd_chat_api::StreamSubscribeParams { chat_id })
        .await
        .unwrap();

    // Set up event notification for client2 to receive streamChunk
    let chunk_received = Arc::new(Notify::new());
    let chunk_received_clone = chunk_received.clone();

    let _cancel_token = client2.on_chat_event(chat_id, move |event| {
        let chunk_received = chunk_received_clone.clone();
        async move {
            if let ChatEvent::StreamChunk(data) = event {
                assert_eq!(data.chat_id, chat_id);
                chunk_received.notify_one();
            }
        }
    });

    // Client1 pushes to stream
    client1
        .stream_push(rhd_chat_api::StreamPushParams {
            chat_id,
            reasoning_content: None,
            content: Some("Hello".to_string()),
            tool_calls: None,
        })
        .await
        .unwrap();

    // Client2 should receive streamChunk event via stream subscription
    let result = tokio::time::timeout(Duration::from_secs(5), chunk_received.notified()).await;
    assert!(result.is_ok(), "Did not receive streamChunk event");
}

/// Test: streamSubscribe returns current state and subscribes to future chunks.
#[tokio::test]
async fn test_stream_subscribe_returns_state() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;
    let create_result = client
        .create_chat(CreateChatParams {
            title: "Test".to_string(),
            tags: vec![],
        })
        .await
        .unwrap();
    let chat_id = create_result.chat_id;

    // Push some content first
    client
        .stream_push(rhd_chat_api::StreamPushParams {
            chat_id,
            reasoning_content: Some("Thinking...".to_string()),
            content: Some("Hello".to_string()),
            tool_calls: None,
        })
        .await
        .unwrap();

    // Subscribe to stream
    let result = client
        .stream_subscribe(rhd_chat_api::StreamSubscribeParams { chat_id })
        .await
        .unwrap();

    assert_eq!(result.reasoning_content, "Thinking...");
    assert_eq!(result.content, "Hello");
    assert!(!result.is_finished);
}

/// Test: streamFinish delivers streamFinished event to stream subscribers.
#[tokio::test]
async fn test_stream_finish_delivers_to_subscribers() {
    let (port, _handle) = start_test_server().await;
    let client1 = connect_client(port).await;
    let client2 = connect_client(port).await;
    let create_result = client1
        .create_chat(CreateChatParams {
            title: "Test".to_string(),
            tags: vec![],
        })
        .await
        .unwrap();
    let chat_id = create_result.chat_id;

    // Client2 subscribes to stream (not just chat)
    let subscribe_result = client2
        .stream_subscribe(rhd_chat_api::StreamSubscribeParams { chat_id })
        .await
        .unwrap();
    assert!(!subscribe_result.is_finished);

    // Set up event notification for client2 to receive streamFinished
    let finished_received = Arc::new(Notify::new());
    let finished_received_clone = finished_received.clone();

    let _cancel_token = client2.on_chat_event(chat_id, move |event| {
        let finished_received = finished_received_clone.clone();
        async move {
            if let ChatEvent::StreamFinished(data) = event {
                assert_eq!(data.chat_id, chat_id);
                finished_received.notify_one();
            }
        }
    });

    // Push some content
    client1
        .stream_push(rhd_chat_api::StreamPushParams {
            chat_id,
            reasoning_content: None,
            content: Some("Hello".to_string()),
            tool_calls: None,
        })
        .await
        .unwrap();

    // Finish stream
    client1
        .stream_finish(rhd_chat_api::StreamFinishParams {
            chat_id,
            reasoning_content: None,
            content: None,
            tool_calls: None,
        })
        .await
        .unwrap();

    // Client2 should receive streamFinished event via stream subscription
    let result = tokio::time::timeout(Duration::from_secs(5), finished_received.notified()).await;
    assert!(result.is_ok(), "Did not receive streamFinished event");
}

/// Test: Message with isStreaming flag is correctly persisted and retrieved.
#[tokio::test]
async fn test_message_streaming_flags_persisted() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;
    let create_result = client
        .create_chat(CreateChatParams {
            title: "Test".to_string(),
            tags: vec![],
        })
        .await
        .unwrap();
    let chat_id = create_result.chat_id;

    // Add message with streaming flags
    let add_result = client
        .add_message(AddMessageParams {
            chat_id,
            role: "assistant".to_string(),
            content: "".to_string(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
            is_finished: false,
            is_streaming: true,
        })
        .await
        .unwrap();

    let message_id = add_result.message_id;

    // Get chat and verify message flags
    let chat_result = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .unwrap();

    let msg = chat_result
        .messages
        .iter()
        .find(|m| m.id == message_id)
        .unwrap();
    assert!(!msg.is_finished);
    assert!(msg.is_streaming);

    // Update message to finished
    client
        .update_message(rhd_chat_api::UpdateMessageParams {
            message_id,
            content: Some("Final content".to_string()),
            reasoning_content: None,
            role: None,
            add_tags: vec![],
            remove_tags: vec![],
            is_finished: Some(true),
            is_streaming: Some(false),
            tool_calls: None,
        })
        .await
        .unwrap();

    // Verify updated flags
    let chat_result2 = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .unwrap();

    let msg2 = chat_result2
        .messages
        .iter()
        .find(|m| m.id == message_id)
        .unwrap();
    assert!(msg2.is_finished);
    assert!(!msg2.is_streaming);
    assert_eq!(msg2.content, "Final content");
}

#[tokio::test]
async fn test_add_tool_message_requires_tool_call_id() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;

    let chat_id = client
        .create_chat(CreateChatParams { title: "t".into(), tags: vec![] })
        .await
        .unwrap()
        .chat_id;

    // Without toolCallId → invalid_request error
    let err = client
        .add_message(AddMessageParams {
            chat_id,
            role: "tool".to_string(),
            content: "result".to_string(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .expect_err("tool message without toolCallId must be rejected");
    assert!(format!("{err:?}").contains("toolCallId") || format!("{err:?}").contains("Invalid"));

    // With toolCallId → stored and returned
    let ok = client
        .add_message(AddMessageParams {
            chat_id,
            role: "tool".to_string(),
            content: "result".to_string(),
            tool_call_id: Some("call_1".to_string()),
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .unwrap();

    let chat_result = client
        .get_chat(GetChatParams { chat_id, if_version_higher_than: None })
        .await
        .unwrap();
    let tool_msg = chat_result
        .messages
        .iter()
        .find(|m| m.id == ok.message_id)
        .expect("tool message present");
    assert_eq!(tool_msg.tool_call_id.as_deref(), Some("call_1"));
}
