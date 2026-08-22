//! Subscription manager for tracking client subscriptions and broadcasting events.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use rhd_chat_api::protocol::Event;

/// Unique identifier for a WebSocket connection.
pub type ConnectionId = String;

/// Channel for sending outgoing messages to a connection.
pub type OutgoingSender = mpsc::UnboundedSender<String>;

/// Subscription manager that tracks client subscriptions.
#[derive(Default)]
pub struct SubscriptionManager {
    /// Map from connection ID to outgoing message sender channel.
    connections: HashMap<ConnectionId, OutgoingSender>,
    /// Map from chat ID to set of subscribed connection IDs.
    chat_subscribers: HashMap<i64, HashSet<ConnectionId>>,
    /// Set of connection IDs subscribed to the chats list.
    chats_list_subscribers: HashSet<ConnectionId>,
    /// Set of connection IDs subscribed to the plugins list.
    plugins_list_subscribers: HashSet<ConnectionId>,
}

impl SubscriptionManager {
    /// Create a new subscription manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new connection and return its ID and outgoing message receiver.
    pub fn register_connection(&mut self) -> (ConnectionId, mpsc::UnboundedReceiver<String>) {
        let connection_id = Uuid::new_v4().to_string();
        let (sender, receiver) = mpsc::unbounded_channel();
        self.connections.insert(connection_id.clone(), sender);
        (connection_id, receiver)
    }

    /// Unregister a connection and remove all its subscriptions.
    pub fn unregister_connection(&mut self, connection_id: &str) {
        // Remove from chat subscriptions
        for subscribers in self.chat_subscribers.values_mut() {
            subscribers.remove(connection_id);
        }
        // Remove from chats list subscriptions
        self.chats_list_subscribers.remove(connection_id);
        // Remove from plugins list subscriptions
        self.plugins_list_subscribers.remove(connection_id);
        // Remove connection
        self.connections.remove(connection_id);
    }

    /// Get the outgoing message sender for a connection.
    pub fn get_sender(&self, connection_id: &str) -> Option<OutgoingSender> {
        self.connections.get(connection_id).cloned()
    }

    /// Subscribe a connection to a specific chat.
    pub fn subscribe_chat(&mut self, connection_id: &str, chat_id: i64) {
        self.chat_subscribers
            .entry(chat_id)
            .or_insert_with(HashSet::new)
            .insert(connection_id.to_string());
    }

    /// Unsubscribe a connection from a specific chat.
    pub fn unsubscribe_chat(&mut self, connection_id: &str, chat_id: i64) {
        if let Some(subscribers) = self.chat_subscribers.get_mut(&chat_id) {
            subscribers.remove(connection_id);
            // Clean up empty sets
            if subscribers.is_empty() {
                self.chat_subscribers.remove(&chat_id);
            }
        }
    }

    /// Subscribe a connection to the chats list.
    pub fn subscribe_chats_list(&mut self, connection_id: &str) {
        self.chats_list_subscribers.insert(connection_id.to_string());
    }

    /// Unsubscribe a connection from the chats list.
    pub fn unsubscribe_chats_list(&mut self, connection_id: &str) {
        self.chats_list_subscribers.remove(connection_id);
    }

    /// Broadcast an event to all subscribers of a specific chat.
    pub fn broadcast_to_chat(&self, chat_id: i64, event: Event) {
        if let Some(subscribers) = self.chat_subscribers.get(&chat_id) {
            let event_json = match serde_json::to_string(&event) {
                Ok(j) => j,
                Err(_) => return,
            };
            for connection_id in subscribers {
                if let Some(sender) = self.connections.get(connection_id) {
                    let _ = sender.send(event_json.clone());
                }
            }
        }
    }

    /// Broadcast an event to all subscribers of the chats list.
    pub fn broadcast_to_chats_list(&self, event: Event) {
        let event_json = match serde_json::to_string(&event) {
            Ok(j) => j,
            Err(_) => return,
        };
        for connection_id in &self.chats_list_subscribers {
            if let Some(sender) = self.connections.get(connection_id) {
                let _ = sender.send(event_json.clone());
            }
        }
    }

    /// Broadcast an event to both chat subscribers and chats list subscribers.
    pub fn broadcast_to_chat_and_list(&self, chat_id: i64, event: Event) {
        self.broadcast_to_chat(chat_id, event.clone());
        self.broadcast_to_chats_list(event);
    }

    /// Subscribe a connection to the plugins list.
    pub fn subscribe_plugins_list(&mut self, connection_id: &str) {
        self.plugins_list_subscribers.insert(connection_id.to_string());
    }

    /// Unsubscribe a connection from the plugins list.
    pub fn unsubscribe_plugins_list(&mut self, connection_id: &str) {
        self.plugins_list_subscribers.remove(connection_id);
    }

    /// Broadcast an event to all subscribers of the plugins list.
    pub fn broadcast_to_plugins_list(&self, event: Event) {
        let event_json = match serde_json::to_string(&event) {
            Ok(j) => j,
            Err(_) => return,
        };
        for connection_id in &self.plugins_list_subscribers {
            if let Some(sender) = self.connections.get(connection_id) {
                let _ = sender.send(event_json.clone());
            }
        }
    }

    /// Broadcast an event to ALL connected clients.
    pub fn broadcast_to_all(&self, event: Event) {
        let event_json = match serde_json::to_string(&event) {
            Ok(j) => j,
            Err(_) => return,
        };
        for sender in self.connections.values() {
            let _ = sender.send(event_json.clone());
        }
    }

    /// Send an event to a specific connection.
    pub fn send_to_connection(&self, connection_id: &str, event: Event) {
        let event_json = match serde_json::to_string(&event) {
            Ok(j) => j,
            Err(_) => return,
        };
        if let Some(sender) = self.connections.get(connection_id) {
            let _ = sender.send(event_json);
        }
    }
}

/// Thread-safe wrapper for subscription manager.
pub type SharedSubscriptionManager = Arc<RwLock<SubscriptionManager>>;

/// Create a new shared subscription manager.
pub fn new_shared_subscription_manager() -> SharedSubscriptionManager {
    Arc::new(RwLock::new(SubscriptionManager::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscription_manager_chat_subscription() {
        let mut manager = SubscriptionManager::new();
        
        // Register connection
        let (conn_id, mut receiver) = manager.register_connection();
        
        // Subscribe to chat
        manager.subscribe_chat(&conn_id, 1);
        
        // Broadcast event
        let event = Event::new("test", serde_json::json!({}));
        manager.broadcast_to_chat(1, event.clone());
        
        // Verify event received
        let received = receiver.recv().await.unwrap();
        assert_eq!(received, serde_json::to_string(&event).unwrap());
        
        // Unsubscribe
        manager.unsubscribe_chat(&conn_id, 1);
        manager.broadcast_to_chat(1, event);
        
        // Verify no event received (channel should be empty)
        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_subscription_manager_chats_list_subscription() {
        let mut manager = SubscriptionManager::new();
        
        let (conn_id, mut receiver) = manager.register_connection();
        manager.subscribe_chats_list(&conn_id);
        
        let event = Event::new("chatCreated", serde_json::json!({}));
        manager.broadcast_to_chats_list(event.clone());
        
        let received = receiver.recv().await.unwrap();
        assert_eq!(received, serde_json::to_string(&event).unwrap());
    }

    #[tokio::test]
    async fn test_subscription_manager_plugins_list_subscription() {
        let mut manager = SubscriptionManager::new();
        
        let (conn_id, mut receiver) = manager.register_connection();
        manager.subscribe_plugins_list(&conn_id);
        
        let event = Event::new("pluginRegistered", serde_json::json!({}));
        manager.broadcast_to_plugins_list(event.clone());
        
        let received = receiver.recv().await.unwrap();
        assert_eq!(received, serde_json::to_string(&event).unwrap());
    }

    #[tokio::test]
    async fn test_subscription_manager_broadcast_to_all() {
        let mut manager = SubscriptionManager::new();
        
        let (_conn_id1, mut receiver1) = manager.register_connection();
        let (_conn_id2, mut receiver2) = manager.register_connection();
        
        let event = Event::new("customEvent", serde_json::json!({}));
        manager.broadcast_to_all(event.clone());
        
        // Both connections should receive the event
        let received1 = receiver1.recv().await.unwrap();
        let received2 = receiver2.recv().await.unwrap();
        assert_eq!(received1, serde_json::to_string(&event).unwrap());
        assert_eq!(received2, serde_json::to_string(&event).unwrap());
    }

    #[tokio::test]
    async fn test_subscription_manager_cleanup_on_disconnect() {
        let mut manager = SubscriptionManager::new();
        
        let (conn_id, _receiver) = manager.register_connection();
        manager.subscribe_chat(&conn_id, 1);
        manager.subscribe_chats_list(&conn_id);
        manager.subscribe_plugins_list(&conn_id);
        
        // Unregister
        manager.unregister_connection(&conn_id);
        
        // Verify all subscriptions are cleaned up
        assert!(manager.chat_subscribers.get(&1).is_none() ||
                !manager.chat_subscribers.get(&1).unwrap().contains(&conn_id));
        assert!(!manager.chats_list_subscribers.contains(&conn_id));
        assert!(!manager.plugins_list_subscribers.contains(&conn_id));
    }
}
