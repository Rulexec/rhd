use crate::ChatDb;

#[test]
fn test_chat_tags() {
    let db = ChatDb::new(":memory:").unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();
    
    // Initially no tags
    let tags = db.get_chat_tags(chat_id).unwrap();
    assert!(tags.is_empty());
    
    // Set tags
    db.set_chat_tags(chat_id, &["tag1".to_string(), "tag2".to_string()]).unwrap();
    let tags = db.get_chat_tags(chat_id).unwrap();
    assert_eq!(tags, vec!["tag1", "tag2"]);
    
    // Add tags
    db.add_chat_tags(chat_id, &["tag3".to_string()]).unwrap();
    let tags = db.get_chat_tags(chat_id).unwrap();
    assert_eq!(tags, vec!["tag1", "tag2", "tag3"]);
    
    // Add duplicate tag (no-op)
    db.add_chat_tags(chat_id, &["tag1".to_string()]).unwrap();
    let tags = db.get_chat_tags(chat_id).unwrap();
    assert_eq!(tags, vec!["tag1", "tag2", "tag3"]);
    
    // Remove tags
    db.remove_chat_tags(chat_id, &["tag2".to_string()]).unwrap();
    let tags = db.get_chat_tags(chat_id).unwrap();
    assert_eq!(tags, vec!["tag1", "tag3"]);
    
    // Get chats by tag
    let chat_ids = db.get_chats_by_tag("tag1").unwrap();
    assert_eq!(chat_ids, vec![chat_id]);
}

#[test]
fn test_message_tags() {
    let db = ChatDb::new(":memory:").unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();
    let message_id = db.add_message(chat_id, "user", "Hello", None, None).unwrap();
    
    // Initially no tags
    let tags = db.get_message_tags(message_id).unwrap();
    assert!(tags.is_empty());
    
    // Set tags
    db.set_message_tags(message_id, &["important".to_string()]).unwrap();
    let tags = db.get_message_tags(message_id).unwrap();
    assert_eq!(tags, vec!["important"]);
    
    // Add tags
    db.add_message_tags(message_id, &["urgent".to_string()]).unwrap();
    let tags = db.get_message_tags(message_id).unwrap();
    assert_eq!(tags, vec!["important", "urgent"]);
    
    // Remove tags
    db.remove_message_tags(message_id, &["important".to_string()]).unwrap();
    let tags = db.get_message_tags(message_id).unwrap();
    assert_eq!(tags, vec!["urgent"]);
}

#[test]
fn test_plugins() {
    let db = ChatDb::new(":memory:").unwrap();
    
    // Initially no plugins
    let plugins = db.get_plugins().unwrap();
    assert!(plugins.is_empty());
    
    // Register plugin
    db.register_plugin("plugin-1").unwrap();
    let plugins = db.get_plugins().unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].plugin_id, "plugin-1");
    assert!(plugins[0].is_active);
    
    // Check if active
    assert!(db.is_plugin_active("plugin-1").unwrap());
    
    // Deactivate plugin
    db.deactivate_plugin("plugin-1").unwrap();
    assert!(!db.is_plugin_active("plugin-1").unwrap());
    
    // Re-register (marks as active)
    db.register_plugin("plugin-1").unwrap();
    assert!(db.is_plugin_active("plugin-1").unwrap());
    
    // Remove plugin
    db.remove_plugin("plugin-1").unwrap();
    let plugins = db.get_plugins().unwrap();
    assert!(plugins.is_empty());
}

#[test]
fn test_custom_events() {
    let db = ChatDb::new(":memory:").unwrap();
    
    // Register a plugin first
    db.register_plugin("sender-plugin").unwrap();
    db.register_plugin("receiver-plugin").unwrap();
    
    // Create custom event
    db.create_custom_event(
        "event-1",
        "my-event",
        Some("sender-plugin"),
        Some("{\"key\": \"value\"}"),
    ).unwrap();
    
    // Get event
    let event = db.get_custom_event("event-1").unwrap().unwrap();
    assert_eq!(event.event_id, "event-1");
    assert_eq!(event.event_name, "my-event");
    assert_eq!(event.sender_plugin_id, Some("sender-plugin".to_string()));
    
    // Check pending events for receiver
    let pending = db.get_pending_events_for_plugin("receiver-plugin").unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].event_id, "event-1");
    
    // Acknowledge event
    db.ack_custom_event("event-1", "receiver-plugin").unwrap();
    
    // Check pending again (should be empty)
    let pending = db.get_pending_events_for_plugin("receiver-plugin").unwrap();
    assert!(pending.is_empty());
    
    // Check if acknowledged
    assert!(db.has_plugin_acked("event-1", "receiver-plugin").unwrap());
    
    // Delete event
    db.delete_custom_event("event-1").unwrap();
    let event = db.get_custom_event("event-1").unwrap();
    assert!(event.is_none());
}
