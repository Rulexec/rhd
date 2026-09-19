//! Integration tests for positional queue insertion (`beforeMessageId`) over WebSocket.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::{Mutex, Notify};
use rhd_db::ChatDb;

use rhd_chat_api::{
    AddQueueMessageParams, CreateChatParams, GetQueueMessagesParams, QueueMessageAddedData,
    SubscribeChatParams,
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

async fn connect_client(port: u16) -> ChatClient {
    let url = format!("ws://127.0.0.1:{}/", port);
    ChatClient::connect(&url).await.unwrap()
}

fn queue_params(chat_id: i64, content: &str, before_message_id: Option<i64>) -> AddQueueMessageParams {
    AddQueueMessageParams {
        chat_id,
        role: "user".to_string(),
        content: content.to_string(),
        tool_call_id: None,
        reasoning_content: None,
        tags: vec![],
        before_message_id,
    }
}

/// Positional insert lands the new message before the anchor, and the
/// `queueMessageAdded` event carries the full inserted message payload.
#[tokio::test]
async fn test_add_queue_message_positional() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;

    let chat_id = client
        .create_chat(CreateChatParams {
            title: "Positional Queue".to_string(),
            tags: vec![],
        })
        .await
        .unwrap()
        .chat_id;

    // Subscribe and collect queueMessageAdded events.
    client
        .subscribe_chat(SubscribeChatParams { chat_id })
        .await
        .unwrap();
    let events = Arc::new(Mutex::new(Vec::<QueueMessageAddedData>::new()));
    let events_cb = events.clone();
    let event_received = Arc::new(Notify::new());
    let received_cb = event_received.clone();
    let _cancel_token = client.on_chat_event(chat_id, move |event| {
        let events = events_cb.clone();
        let event_received = received_cb.clone();
        async move {
            if let ChatEvent::QueueMessageAdded(data) = event {
                events.lock().await.push(data);
                event_received.notify_one();
            }
        }
    });

    let first = client
        .add_queue_message(queue_params(chat_id, "first", None))
        .await
        .unwrap()
        .message_id;
    let second = client
        .add_queue_message(queue_params(chat_id, "second", None))
        .await
        .unwrap()
        .message_id;

    // Insert "inserted" directly before "second".
    let inserted = client
        .add_queue_message(queue_params(chat_id, "inserted", Some(second)))
        .await
        .unwrap()
        .message_id;

    // getQueueMessages returns the inserted message between the two appends.
    let queue = client
        .get_queue_messages(GetQueueMessagesParams { chat_id })
        .await
        .unwrap()
        .messages;
    assert_eq!(
        queue.iter().map(|m| m.id).collect::<Vec<_>>(),
        vec![first, inserted, second]
    );
    assert_eq!(
        queue.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(),
        vec!["first", "inserted", "second"]
    );

    // The queueMessageAdded event payload matches the inserted message.
    let mut found = None;
    for _ in 0..10 {
        if let Some(data) = events.lock().await.iter().find(|d| d.message.id == inserted) {
            found = Some(data.clone());
            break;
        }
        tokio::time::timeout(Duration::from_millis(500), event_received.notified())
            .await
            .ok();
    }
    let data = found.expect("queueMessageAdded event for the positionally inserted message");
    assert_eq!(data.chat_id, chat_id);
    assert_eq!(data.message.id, inserted);
    assert_eq!(data.message.content, "inserted");
    assert_eq!(data.message.chat_id, chat_id);
}

/// Unknown `beforeMessageId` → MESSAGE_NOT_FOUND; an anchor from another chat
/// → INVALID_REQUEST. Neither creates a queue message.
#[tokio::test]
async fn test_add_queue_message_positional_errors() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;

    let chat1 = client
        .create_chat(CreateChatParams {
            title: "Chat 1".to_string(),
            tags: vec![],
        })
        .await
        .unwrap()
        .chat_id;
    let chat2 = client
        .create_chat(CreateChatParams {
            title: "Chat 2".to_string(),
            tags: vec![],
        })
        .await
        .unwrap()
        .chat_id;

    let foreign = client
        .add_queue_message(queue_params(chat2, "foreign anchor", None))
        .await
        .unwrap()
        .message_id;

    // Unknown anchor id → message-not-found class error.
    let err = client
        .add_queue_message(queue_params(chat1, "ghost insert", Some(999_999)))
        .await
        .expect_err("unknown beforeMessageId must be rejected");
    assert!(
        matches!(&err, ClientError::Server { code, .. } if code == "MESSAGE_NOT_FOUND"),
        "expected MESSAGE_NOT_FOUND, got {err:?}"
    );

    // Anchor belonging to another chat → invalid request.
    let err = client
        .add_queue_message(queue_params(chat1, "cross-chat insert", Some(foreign)))
        .await
        .expect_err("foreign-chat beforeMessageId must be rejected");
    assert!(
        matches!(&err, ClientError::Server { code, .. } if code == "INVALID_REQUEST"),
        "expected INVALID_REQUEST, got {err:?}"
    );

    // Chat 1's queue stayed empty; chat 2's queue only holds the anchor.
    assert!(client
        .get_queue_messages(GetQueueMessagesParams { chat_id: chat1 })
        .await
        .unwrap()
        .messages
        .is_empty());
    let queue2 = client
        .get_queue_messages(GetQueueMessagesParams { chat_id: chat2 })
        .await
        .unwrap()
        .messages;
    assert_eq!(queue2.len(), 1);
    assert_eq!(queue2[0].id, foreign);
}
