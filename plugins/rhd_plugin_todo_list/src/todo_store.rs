//! Per-chat todo list storage.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::parser::TodoItem;

/// Thread-safe storage for todo lists per chat.
#[derive(Debug, Clone)]
pub struct TodoStore {
    /// Map from chat_id to todo items.
    chats: Arc<RwLock<HashMap<i64, Vec<TodoItem>>>>,
}

impl TodoStore {
    /// Create a new empty todo store.
    pub fn new() -> Self {
        Self {
            chats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the todo list for a chat.
    pub async fn get(&self, chat_id: i64) -> Option<Vec<TodoItem>> {
        let chats = self.chats.read().await;
        chats.get(&chat_id).cloned()
    }

    /// Set the todo list for a chat (replaces existing).
    pub async fn set(&self, chat_id: i64, items: Vec<TodoItem>) {
        let mut chats = self.chats.write().await;
        chats.insert(chat_id, items);
    }

    /// Clear the todo list for a chat.
    pub async fn clear(&self, chat_id: i64) {
        let mut chats = self.chats.write().await;
        chats.remove(&chat_id);
    }

    /// Check if a chat has a todo list.
    pub async fn has(&self, chat_id: i64) -> bool {
        let chats = self.chats.read().await;
        chats.contains_key(&chat_id)
    }

    /// Get all chat IDs with todo lists.
    pub async fn chat_ids(&self) -> Vec<i64> {
        let chats = self.chats.read().await;
        chats.keys().cloned().collect()
    }
}

impl Default for TodoStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::TodoStatus;

    #[tokio::test]
    async fn test_store_set_and_get() {
        let store = TodoStore::new();
        let items = vec![
            TodoItem::new("Task 1".to_string(), TodoStatus::Pending),
            TodoItem::new("Task 2".to_string(), TodoStatus::Completed),
        ];
        
        store.set(1, items.clone()).await;
        
        let retrieved = store.get(1).await;
        assert_eq!(retrieved, Some(items));
    }

    #[tokio::test]
    async fn test_store_get_nonexistent() {
        let store = TodoStore::new();
        let retrieved = store.get(999).await;
        assert_eq!(retrieved, None);
    }

    #[tokio::test]
    async fn test_store_clear() {
        let store = TodoStore::new();
        let items = vec![TodoItem::new("Task".to_string(), TodoStatus::Pending)];
        
        store.set(1, items).await;
        assert!(store.has(1).await);
        
        store.clear(1).await;
        assert!(!store.has(1).await);
    }

    #[tokio::test]
    async fn test_store_replace() {
        let store = TodoStore::new();
        
        let items1 = vec![TodoItem::new("Task 1".to_string(), TodoStatus::Pending)];
        store.set(1, items1).await;
        
        let items2 = vec![
            TodoItem::new("Task A".to_string(), TodoStatus::InProgress),
            TodoItem::new("Task B".to_string(), TodoStatus::Completed),
        ];
        store.set(1, items2.clone()).await;
        
        let retrieved = store.get(1).await;
        assert_eq!(retrieved, Some(items2));
    }

    #[tokio::test]
    async fn test_store_chat_ids() {
        let store = TodoStore::new();
        
        store.set(1, vec![]).await;
        store.set(2, vec![]).await;
        store.set(3, vec![]).await;
        
        let mut ids = store.chat_ids().await;
        ids.sort();
        assert_eq!(ids, vec![1, 2, 3]);
    }
}
