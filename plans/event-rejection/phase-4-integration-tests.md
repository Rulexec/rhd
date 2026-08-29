# Phase 4: Integration Tests

## Overview

This phase creates comprehensive integration tests that verify the complete flow of both features: rejection and context fields. Tests will verify end-to-end behavior across the entire stack.

**Scope**: Integration tests in `rhd_chat_server` and database tests in `rhd_db`
**Out of scope**: Unit tests (covered in previous phases), client implementations

## Files to Modify

### 1. `packages/rhd_chat_server/tests/websocket_tests.rs`

**Add test for rejection flow** (after line 533):

```rust
#[tokio::test]
async fn test_custom_event_rejection_flow() {
    let (port, _handle) = start_test_server().await;

    // Connect three clients
    let client1 = connect_client(port).await;
    let client2 = connect_client(port).await;
    let client3 = connect_client(port).await;

    // Register plugins on all connections
    client1
        .register_plugin(RegisterPluginParams {
            plugin_id: "sender".to_string(),
        })
        .await
        .unwrap();
    client2
        .register_plugin(RegisterPluginParams {
            plugin_id: "rejector".to_string(),
        })
        .await
        .unwrap();
    client3
        .register_plugin(RegisterPluginParams {
            plugin_id: "acceptor".to_string(),
        })
        .await
        .unwrap();

    // Set up event notification for client2 and client3 to receive customEvent
    let event_received_2 = Arc::new(Notify::new());
    let event_received_2_clone = event_received_2.clone();
    let received_event_id_2 = Arc::new(Mutex::new(String::new()));
    let received_event_id_2_clone = received_event_id_2.clone();

    let _cancel_token_2 = client2.on_custom_event(move |event| {
        let event_received = event_received_2_clone.clone();
        let received_event_id = received_event_id_2_clone.clone();
        async move {
            *received_event_id.lock().await = event.event_id.clone();
            event_received.notify_one();
        }
    });

    let event_received_3 = Arc::new(Notify::new());
    let event_received_3_clone = event_received_3.clone();
    let received_event_id_3 = Arc::new(Mutex::new(String::new()));
    let received_event_id_3_clone = received_event_id_3.clone();

    let _cancel_token_3 = client3.on_custom_event(move |event| {
        let event_received = event_received_3_clone.clone();
        let received_event_id = received_event_id_3_clone.clone();
        async move {
            *received_event_id.lock().await = event.event_id.clone();
            event_received.notify_one();
        }
    });

    // Client1 sends custom event
    let send_result = client1
        .send_custom_event(SendCustomEventParams {
            event_name: "test-event".to_string(),
            additional: Some("{\"key\": \"value\"}".to_string()),
            chat_id: None,
            message_id: None,
            tool_call_id: None,
        })
        .await
        .unwrap();
    let event_id = send_result.event_id.clone();
    let event_id_for_ack = send_result.event_id;

    // Client2 and client3 should receive customEvent
    let result = tokio::time::timeout(Duration::from_secs(5), event_received_2.notified()).await;
    assert!(result.is_ok(), "Client2 did not receive customEvent");
    assert_eq!(*received_event_id_2.lock().await, event_id);

    let result = tokio::time::timeout(Duration::from_secs(5), event_received_3.notified()).await;
    assert!(result.is_ok(), "Client3 did not receive customEvent");
    assert_eq!(*received_event_id_3.lock().await, event_id);

    // Set up acknowledgment notification for client1
    let ack_received = Arc::new(Mutex::new(Vec::new()));
    let ack_received_clone = ack_received.clone();
    let all_acks_received = Arc::new(Notify::new());
    let all_acks_received_clone = all_acks_received.clone();

    let _cancel_token = client1.on_custom_event_acknowledged(move |event| {
        let ack_received = ack_received_clone.clone();
        let all_acks_received = all_acks_received_clone.clone();
        async move {
            ack_received.lock().await.push(event);
            if ack_received.lock().await.len() == 2 {
                all_acks_received.notify_one();
            }
        }
    });

    // Client2: reject event
    client2
        .ack_custom_event(AckCustomEventParams {
            event_id: event_id_for_ack.clone(),
            is_rejected: Some(true),
        })
        .await
        .unwrap();

    // Client3: accept event
    client3
        .ack_custom_event(AckCustomEventParams {
            event_id: event_id_for_ack,
            is_rejected: Some(false),
        })
        .await
        .unwrap();

    // Client1 should receive both acknowledgments
    let result = tokio::time::timeout(Duration::from_secs(5), all_acks_received.notified()).await;
    assert!(result.is_ok(), "Did not receive all acknowledgments");

    let acks = ack_received.lock().await;
    assert_eq!(acks.len(), 2);

    // Verify one is rejected and one is accepted
    let rejected = acks.iter().find(|a| a.is_rejected);
    let accepted = acks.iter().find(|a| !a.is_rejected);

    assert!(rejected.is_some(), "Should have one rejected acknowledgment");
    assert!(accepted.is_some(), "Should have one accepted acknowledgment");

    assert_eq!(rejected.unwrap().acknowledging_plugin_id, "rejector");
    assert_eq!(accepted.unwrap().acknowledging_plugin_id, "acceptor");
}
```

**Add test for context fields** (after the rejection test):

```rust
#[tokio::test]
async fn test_custom_event_with_context_fields() {
    let (port, _handle) = start_test_server().await;

    // Connect both clients
    let client1 = connect_client(port).await;
    let client2 = connect_client(port).await;

    // Register plugins
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
    let received_event = Arc::new(Mutex::new(None));
    let received_event_clone = received_event.clone();

    let _cancel_token = client2.on_custom_event(move |event| {
        let event_received = event_received_clone.clone();
        let received_event = received_event_clone.clone();
        async move {
            *received_event.lock().await = Some(event);
            event_received.notify_one();
        }
    });

    // Client1 sends custom event with context fields
    let send_result = client1
        .send_custom_event(SendCustomEventParams {
            event_name: "test-event".to_string(),
            additional: Some("{\"key\": \"value\"}".to_string()),
            chat_id: Some("chat-123".to_string()),
            message_id: Some("message-456".to_string()),
            tool_call_id: Some("call-789".to_string()),
        })
        .await
        .unwrap();

    // Client2 should receive customEvent with context fields
    let result = tokio::time::timeout(Duration::from_secs(5), event_received.notified()).await;
    assert!(result.is_ok(), "Did not receive customEvent");

    let event = received_event.lock().await.take().unwrap();
    assert_eq!(event.event_id, send_result.event_id);
    assert_eq!(event.event_name, "test-event");
    assert_eq!(event.chat_id, Some("chat-123".to_string()));
    assert_eq!(event.message_id, Some("message-456".to_string()));
    assert_eq!(event.tool_call_id, Some("call-789".to_string()));
}
```

**Add test for context fields in recovery** (after the context fields test):

```rust
#[tokio::test]
async fn test_custom_event_context_fields_in_recovery() {
    let (port, _handle) = start_test_server().await;

    // Connect sender and send event with context fields
    let sender = connect_client(port).await;
    sender
        .register_plugin(RegisterPluginParams {
            plugin_id: "sender".to_string(),
        })
        .await
        .unwrap();

    let send_result = sender
        .send_custom_event(SendCustomEventParams {
            event_name: "test-event".to_string(),
            additional: Some("{\"key\": \"value\"}".to_string()),
            chat_id: Some("chat-123".to_string()),
            message_id: Some("message-456".to_string()),
            tool_call_id: Some("call-789".to_string()),
        })
        .await
        .unwrap();

    // Connect receiver and get pending acks
    let receiver = connect_client(port).await;
    receiver
        .register_plugin(RegisterPluginParams {
            plugin_id: "receiver".to_string(),
        })
        .await
        .unwrap();

    let pending = receiver.get_pending_acks().await.unwrap();
    assert_eq!(pending.pending_events.len(), 1);

    let pending_event = &pending.pending_events[0];
    assert_eq!(pending_event.event_id, send_result.event_id);
    assert_eq!(pending_event.event_name, "test-event");
    assert_eq!(pending_event.chat_id, Some("chat-123".to_string()));
    assert_eq!(pending_event.message_id, Some("message-456".to_string()));
    assert_eq!(pending_event.tool_call_id, Some("call-789".to_string()));
}
```

**Add test for combined scenario** (after the recovery test):

```rust
#[tokio::test]
async fn test_custom_event_context_fields_with_rejection() {
    let (port, _handle) = start_test_server().await;

    // Connect three clients
    let client1 = connect_client(port).await;
    let client2 = connect_client(port).await;
    let client3 = connect_client(port).await;

    // Register plugins
    client1
        .register_plugin(RegisterPluginParams {
            plugin_id: "sender".to_string(),
        })
        .await
        .unwrap();
    client2
        .register_plugin(RegisterPluginParams {
            plugin_id: "rejector".to_string(),
        })
        .await
        .unwrap();
    client3
        .register_plugin(RegisterPluginParams {
            plugin_id: "acceptor".to_string(),
        })
        .await
        .unwrap();

    // Set up event notification for client2 and client3
    let event_received_2 = Arc::new(Notify::new());
    let event_received_2_clone = event_received_2.clone();
    let received_event_2 = Arc::new(Mutex::new(None));
    let received_event_2_clone = received_event_2.clone();

    let _cancel_token_2 = client2.on_custom_event(move |event| {
        let event_received = event_received_2_clone.clone();
        let received_event = received_event_2_clone.clone();
        async move {
            *received_event.lock().await = Some(event);
            event_received.notify_one();
        }
    });

    let event_received_3 = Arc::new(Notify::new());
    let event_received_3_clone = event_received_3.clone();

    let _cancel_token_3 = client3.on_custom_event(move |event| {
        let event_received = event_received_3_clone.clone();
        async move {
            event_received.notify_one();
        }
    });

    // Client1 sends custom event with context fields
    let send_result = client1
        .send_custom_event(SendCustomEventParams {
            event_name: "test-event".to_string(),
            additional: None,
            chat_id: Some("chat-123".to_string()),
            message_id: Some("message-456".to_string()),
            tool_call_id: Some("call-789".to_string()),
        })
        .await
        .unwrap();

    // Client2 should receive customEvent with context fields
    let result = tokio::time::timeout(Duration::from_secs(5), event_received_2.notified()).await;
    assert!(result.is_ok(), "Client2 did not receive customEvent");

    let event = received_event_2.lock().await.take().unwrap();
    assert_eq!(event.chat_id, Some("chat-123".to_string()));
    assert_eq!(event.message_id, Some("message-456".to_string()));
    assert_eq!(event.tool_call_id, Some("call-789".to_string()));

    // Wait for client3 to receive event
    let result = tokio::time::timeout(Duration::from_secs(5), event_received_3.notified()).await;
    assert!(result.is_ok(), "Client3 did not receive customEvent");

    // Set up acknowledgment notification for client1
    let ack_received = Arc::new(Mutex::new(Vec::new()));
    let ack_received_clone = ack_received.clone();
    let all_acks_received = Arc::new(Notify::new());
    let all_acks_received_clone = all_acks_received.clone();

    let _cancel_token = client1.on_custom_event_acknowledged(move |event| {
        let ack_received = ack_received_clone.clone();
        let all_acks_received = all_acks_received_clone.clone();
        async move {
            ack_received.lock().await.push(event);
            if ack_received.lock().await.len() == 2 {
                all_acks_received.notify_one();
            }
        }
    });

    // Client2: reject event
    client2
        .ack_custom_event(AckCustomEventParams {
            event_id: send_result.event_id.clone(),
            is_rejected: Some(true),
        })
        .await
        .unwrap();

    // Client3: accept event
    client3
        .ack_custom_event(AckCustomEventParams {
            event_id: send_result.event_id,
            is_rejected: Some(false),
        })
        .await
        .unwrap();

    // Client1 should receive both acknowledgments
    let result = tokio::time::timeout(Duration::from_secs(5), all_acks_received.notified()).await;
    assert!(result.is_ok(), "Did not receive all acknowledgments");

    let acks = ack_received.lock().await;
    assert_eq!(acks.len(), 2);

    // Verify one is rejected and one is accepted
    let rejected = acks.iter().find(|a| a.is_rejected);
    let accepted = acks.iter().find(|a| !a.is_rejected);

    assert!(rejected.is_some(), "Should have one rejected acknowledgment");
    assert!(accepted.is_some(), "Should have one accepted acknowledgment");
}
```

**Add test for backward compatibility** (after the combined test):

```rust
#[tokio::test]
async fn test_custom_event_backward_compatibility() {
    let (port, _handle) = start_test_server().await;

    // Connect both clients
    let client1 = connect_client(port).await;
    let client2 = connect_client(port).await;

    // Register plugins
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
    let received_event = Arc::new(Mutex::new(None));
    let received_event_clone = received_event.clone();

    let _cancel_token = client2.on_custom_event(move |event| {
        let event_received = event_received_clone.clone();
        let received_event = received_event_clone.clone();
        async move {
            *received_event.lock().await = Some(event);
            event_received.notify_one();
        }
    });

    // Client1 sends custom event WITHOUT context fields (old API)
    let send_result = client1
        .send_custom_event(SendCustomEventParams {
            event_name: "test-event".to_string(),
            additional: Some("{\"key\": \"value\"}".to_string()),
            chat_id: None,
            message_id: None,
            tool_call_id: None,
        })
        .await
        .unwrap();

    // Client2 should receive customEvent with None context fields
    let result = tokio::time::timeout(Duration::from_secs(5), event_received.notified()).await;
    assert!(result.is_ok(), "Did not receive customEvent");

    let event = received_event.lock().await.take().unwrap();
    assert_eq!(event.event_id, send_result.event_id);
    assert_eq!(event.chat_id, None);
    assert_eq!(event.message_id, None);
    assert_eq!(event.tool_call_id, None);

    // Set up acknowledgment notification for client1
    let ack_received = Arc::new(Notify::new());
    let ack_received_clone = ack_received.clone();

    let _cancel_token = client1.on_custom_event_acknowledged(move |event| {
        let ack_received = ack_received_clone.clone();
        async move {
            // Verify is_rejected defaults to false
            assert!(!event.is_rejected);
            ack_received.notify_one();
        }
    });

    // Client2: acknowledge event WITHOUT is_rejected field (old API)
    client2
        .ack_custom_event(AckCustomEventParams {
            event_id: send_result.event_id,
            is_rejected: None,
        })
        .await
        .unwrap();

    // Client1 should receive acknowledgment with is_rejected: false
    let result = tokio::time::timeout(Duration::from_secs(5), ack_received.notified()).await;
    assert!(result.is_ok(), "Did not receive customEventAcknowledged");
}
```

### 2. `packages/rhd_db/src/chat_db/tests/tags_plugins_events_tests.rs`

**Add database-level tests** (after line 135):

```rust
#[test]
fn test_custom_events_context_fields_storage() {
    let db = ChatDb::new(":memory:").unwrap();
    
    // Register a plugin first
    db.register_plugin("sender-plugin").unwrap();
    
    // Create custom event with all context fields
    db.create_custom_event(
        "event-1",
        "my-event",
        Some("sender-plugin"),
        Some("{\"key\": \"value\"}"),
        Some("chat-123"),
        Some("message-456"),
        Some("call-789"),
    ).unwrap();
    
    // Get event and verify all context fields are stored
    let event = db.get_custom_event("event-1").unwrap().unwrap();
    assert_eq!(event.event_id, "event-1");
    assert_eq!(event.event_name, "my-event");
    assert_eq!(event.sender_plugin_id, Some("sender-plugin".to_string()));
    assert_eq!(event.additional, Some("{\"key\": \"value\"}".to_string()));
    assert_eq!(event.chat_id, Some("chat-123".to_string()));
    assert_eq!(event.message_id, Some("message-456".to_string()));
    assert_eq!(event.tool_call_id, Some("call-789".to_string()));
}

#[test]
fn test_custom_events_partial_context_fields() {
    let db = ChatDb::new(":memory:").unwrap();
    
    // Register a plugin first
    db.register_plugin("sender-plugin").unwrap();
    
    // Create custom event with only some context fields
    db.create_custom_event(
        "event-1",
        "my-event",
        Some("sender-plugin"),
        None,
        Some("chat-123"),
        None,
        Some("call-789"),
    ).unwrap();
    
    // Get event and verify only provided context fields are stored
    let event = db.get_custom_event("event-1").unwrap().unwrap();
    assert_eq!(event.chat_id, Some("chat-123".to_string()));
    assert_eq!(event.message_id, None);
    assert_eq!(event.tool_call_id, Some("call-789".to_string()));
}

#[test]
fn test_custom_events_pending_includes_context_fields() {
    let db = ChatDb::new(":memory:").unwrap();
    
    // Register plugins
    db.register_plugin("sender-plugin").unwrap();
    db.register_plugin("receiver-plugin").unwrap();
    
    // Create custom event with context fields
    db.create_custom_event(
        "event-1",
        "my-event",
        Some("sender-plugin"),
        None,
        Some("chat-123"),
        Some("message-456"),
        Some("call-789"),
    ).unwrap();
    
    // Get pending events for receiver
    let pending = db.get_pending_events_for_plugin("receiver-plugin").unwrap();
    assert_eq!(pending.len(), 1);
    
    let pending_event = &pending[0];
    assert_eq!(pending_event.event_id, "event-1");
    assert_eq!(pending_event.chat_id, Some("chat-123".to_string()));
    assert_eq!(pending_event.message_id, Some("message-456".to_string()));
    assert_eq!(pending_event.tool_call_id, Some("call-789".to_string()));
}
```

## Implementation Notes

1. **Test isolation**: Each test creates its own plugins and events to avoid interference
2. **Recovery testing**: Tests verify that `getPendingAcks` returns all fields after simulating a disconnection scenario
3. **Multiple acknowledgments**: Tests verify that rejection by one plugin doesn't prevent others from acknowledging
4. **Backward compatibility**: Tests verify that old clients (without new fields) continue to work
5. **Async/await patterns**: Tests use `Arc<Notify>` and `Arc<Mutex>` for thread-safe event notification
6. **Timeout handling**: All event waits use `tokio::time::timeout` to prevent tests from hanging

## Dependencies

- **Phase 3** - Server logic must be complete before integration tests can run
- This is the final phase

## Success Criteria

- ✅ All new tests pass
- ✅ Rejection flow works end-to-end (multiple plugins, mixed accept/reject)
- ✅ Context fields are preserved through the entire lifecycle (send → broadcast → store → recover)
- ✅ Recovery via `getPendingAcks` includes all context fields
- ✅ Combined scenario (context fields + rejection) works correctly
- ✅ Backward compatibility is maintained (old clients work without changes)
- ✅ Existing tests continue to pass (no regressions)
- ✅ Database tests verify context field storage and retrieval
