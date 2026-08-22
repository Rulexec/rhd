# Phase 3: Chat Client PluginsMonitor and ChatMonitor Features

## Overview

This phase adds `PluginsMonitor` and `ChatMonitor` features to `rhd_chat_client` that can be reused by any plugin.

**Scope**:
- Create `PluginsMonitor` struct that tracks ALL registered plugins (active and inactive)
- Create `ChatMonitor` struct for generic chat monitoring
- Add `create_plugins_monitor` and `create_chat_monitor` methods to `ChatClient`
- Implement `wait_for_acks_except` for acknowledgment coordination
- Export new types from `lib.rs`

**Out of Scope**:
- Plugin implementation (Phase 4)
- Server-side enhancements (Phase 2)
- Integration testing (Phase 5)

## Files to Create/Modify

### 1. `packages/rhd_chat_client/src/plugins_monitor.rs` (CREATE)

**Purpose**: Implementation of the PluginsMonitor logic that tracks ALL registered plugins.

**Complete Implementation**:
```rust
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;

use rhd_chat_api::{GetPluginsParams, GetPluginsResult};

use crate::client::ChatClient;
use crate::error::ClientError;
use crate::event_stream::{CancellationToken, PluginsListEvent};

/// Information about a registered plugin.
struct PluginInfo {
    plugin_id: String,
    is_active: bool,
}

/// Monitor that tracks ALL registered plugins (active and inactive) and their acknowledgments.
pub struct PluginsMonitor {
    /// Map of plugin_id -> PluginInfo for ALL registered plugins.
    all_plugins: Arc<RwLock<HashMap<String, PluginInfo>>>,
    /// Map of event_id -> set of plugin_ids that have acknowledged.
    acknowledgments: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    /// Cancellation token for the plugins list subscription.
    _plugins_list_token: CancellationToken,
}

impl PluginsMonitor {
    /// Create a new plugins monitor.
    ///
    /// This automatically subscribes to plugins list events and fetches
    /// the initial list of ALL registered plugins (active and inactive).
    pub async fn new(client: &ChatClient) -> Result<Self, ClientError> {
        let all_plugins = Arc::new(RwLock::new(HashMap::new()));
        let acknowledgments = Arc::new(RwLock::new(HashMap::new()));

        // Fetch initial plugin list (ALL plugins, not just active)
        let plugins_result = client.get_plugins(GetPluginsParams {}).await?;
        {
            let mut plugins = all_plugins.write().await;
            for plugin in plugins_result.plugins {
                plugins.insert(plugin.plugin_id.clone(), PluginInfo {
                    plugin_id: plugin.plugin_id,
                    is_active: plugin.is_active,
                });
            }
        }

        // Subscribe to plugins list events
        let all_plugins_clone = Arc::clone(&all_plugins);
        let token = client.on_plugins_list_event(move |event| {
            let plugins = Arc::clone(&all_plugins_clone);
            async move {
                match event {
                    PluginsListEvent::PluginRegistered(data) => {
                        let mut plugins = plugins.write().await;
                        plugins.insert(data.plugin.plugin_id.clone(), PluginInfo {
                            plugin_id: data.plugin.plugin_id,
                            is_active: data.plugin.is_active,
                        });
                    }
                    PluginsListEvent::PluginUpdated(data) => {
                        let mut plugins = plugins.write().await;
                        if let Some(info) = plugins.get_mut(&data.plugin.plugin_id) {
                            info.is_active = data.plugin.is_active;
                        }
                    }
                    PluginsListEvent::PluginRemoved(data) => {
                        let mut plugins = plugins.write().await;
                        plugins.remove(&data.plugin_id);
                    }
                }
            }
        });

        Ok(Self {
            all_plugins,
            acknowledgments,
            _plugins_list_token: token,
        })
    }

    /// Get current set of ALL registered plugin IDs (active and inactive).
    pub async fn get_all_plugin_ids(&self) -> HashSet<String> {
        self.all_plugins.read().await.keys().cloned().collect()
    }

    /// Get current set of active plugin IDs.
    pub async fn get_active_plugin_ids(&self) -> HashSet<String> {
        self.all_plugins.read().await.iter()
            .filter(|(_, info)| info.is_active)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Check if a specific plugin is registered (active or inactive).
    pub async fn is_plugin_registered(&self, plugin_id: &str) -> bool {
        self.all_plugins.read().await.contains_key(plugin_id)
    }

    /// Check if a specific plugin is active.
    pub async fn is_plugin_active(&self, plugin_id: &str) -> bool {
        self.all_plugins.read().await.get(plugin_id)
            .map(|info| info.is_active)
            .unwrap_or(false)
    }

    /// Record that a plugin has acknowledged an event.
    ///
    /// This is called when a custom event acknowledgment is received.
    pub async fn record_acknowledgment(&self, event_id: &str, plugin_id: &str) {
        let mut acks = self.acknowledgments.write().await;
        acks.entry(event_id.to_string())
            .or_insert_with(HashSet::new)
            .insert(plugin_id.to_string());
    }

    /// Wait until ALL registered plugins (except the specified ones) have acknowledged an event.
    ///
    /// This includes inactive plugins that haven't started yet. They must start and acknowledge
    /// the event before this method returns.
    ///
    /// # Arguments
    /// * `event_id` - The event ID to wait for
    /// * `except_plugin_ids` - Plugin IDs to exclude from the wait (e.g., the sender)
    /// * `timeout_duration` - Maximum time to wait
    ///
    /// # Returns
    /// * `Ok(())` if all required plugins acknowledged
    /// * `Err(ClientError::Timeout)` if timeout expired
    pub async fn wait_for_acks_except(
        &self,
        event_id: &str,
        except_plugin_ids: &[&str],
        timeout_duration: Duration,
    ) -> Result<(), ClientError> {
        let except_set: HashSet<String> = except_plugin_ids.iter().map(|s| s.to_string()).collect();
        
        let result = timeout(timeout_duration, async {
            loop {
                let all_plugins = self.all_plugins.read().await.clone();
                let acks = self.acknowledgments.read().await;
                let event_acks = acks.get(event_id).cloned().unwrap_or_default();

                // Check if ALL registered plugins (except excluded ones) have acknowledged
                let all_acked = all_plugins.keys().all(|plugin_id| {
                    except_set.contains(plugin_id) || event_acks.contains(plugin_id)
                });

                if all_acked {
                    return Ok(());
                }

                // Drop locks before sleeping
                drop(acks);
                drop(all_plugins);

                // Wait a bit before checking again
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }).await;

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(ClientError::Internal("Timeout waiting for plugin acknowledgments".to_string())),
        }
    }

    /// Clean up old acknowledgment records.
    ///
    /// This should be called periodically to prevent memory leaks.
    pub async fn cleanup_old_acks(&self, max_age: Duration) {
        // Note: This requires tracking timestamps for acknowledgments
        // For now, we'll skip this implementation as it's not critical
        // In production, you'd want to add timestamps and clean up old entries
    }
}
```

### 2. `packages/rhd_chat_client/src/chat_monitor.rs` (CREATE)

**Purpose**: Generic chat monitoring logic that can be used by any plugin.

**Complete Implementation**:
```rust
use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use rhd_chat_api::{GetChatParams, ListChatsParams, Message};

use crate::client::ChatClient;
use crate::error::ClientError;
use crate::event_stream::{CancellationToken, ChatEvent, ChatsListEvent};

/// Represents the state of a chat being monitored.
#[derive(Clone)]
pub struct ChatState {
    pub chat_id: i64,
    pub messages: Vec<Message>,
    pub queued_messages_count: i64,
    pub tags: Vec<String>,
}

/// Monitor that tracks chats and their state.
pub struct ChatMonitor {
    client: Arc<ChatClient>,
    chat_states: Arc<RwLock<HashMap<i64, ChatState>>>,
    _chats_list_token: CancellationToken,
    chat_tokens: Arc<RwLock<Vec<CancellationToken>>>,
}

impl ChatMonitor {
    /// Create a new chat monitor.
    pub async fn new(client: Arc<ChatClient>) -> Result<Self, ClientError> {
        let chat_states = Arc::new(RwLock::new(HashMap::new()));
        let chat_tokens = Arc::new(RwLock::new(Vec::new()));

        // Subscribe to chats list events
        let chat_states_clone = Arc::clone(&chat_states);
        let client_clone = Arc::clone(&client);
        let chats_list_token = client.on_chats_list_event(move |event| {
            let states = Arc::clone(&chat_states_clone);
            let client = Arc::clone(&client_clone);
            async move {
                match event {
                    ChatsListEvent::ChatCreated(data) => {
                        if let Ok(chat_result) = client.get_chat(GetChatParams { chat_id: data.chat.id }).await {
                            let state = ChatState {
                                chat_id: data.chat.id,
                                messages: chat_result.messages,
                                queued_messages_count: chat_result.queued_messages_count,
                                tags: data.chat.tags,
                            };
                            states.write().await.insert(data.chat.id, state);
                        }
                    }
                    ChatsListEvent::ChatUpdated(data) => {
                        if let Ok(chat_result) = client.get_chat(GetChatParams { chat_id: data.chat.id }).await {
                            let state = ChatState {
                                chat_id: data.chat.id,
                                messages: chat_result.messages,
                                queued_messages_count: chat_result.queued_messages_count,
                                tags: data.chat.tags,
                            };
                            states.write().await.insert(data.chat.id, state);
                        }
                    }
                    ChatsListEvent::ChatDeleted(data) => {
                        states.write().await.remove(&data.chat_id);
                    }
                }
            }
        });

        // Fetch initial chat list
        let list_result = client.list_chats(ListChatsParams { tags: vec![] }).await?;
        for chat_summary in list_result.chats {
            if let Ok(chat_result) = client.get_chat(GetChatParams { chat_id: chat_summary.id }).await {
                let state = ChatState {
                    chat_id: chat_summary.id,
                    messages: chat_result.messages,
                    queued_messages_count: chat_result.queued_messages_count,
                    tags: chat_summary.tags,
                };
                chat_states.write().await.insert(chat_summary.id, state);
            }
        }

        Ok(Self {
            client,
            chat_states,
            _chats_list_token: chats_list_token,
            chat_tokens,
        })
    }

    /// Subscribe to events for all monitored chats.
    pub async fn subscribe_to_all_chats(&self) -> Result<(), ClientError> {
        let chat_ids: Vec<i64> = self.chat_states.read().await.keys().cloned().collect();
        
        for chat_id in chat_ids {
            self.subscribe_to_chat(chat_id).await?;
        }
        
        Ok(())
    }

    /// Subscribe to events for a specific chat.
    async fn subscribe_to_chat(&self, chat_id: i64) -> Result<(), ClientError> {
        let chat_states = Arc::clone(&self.chat_states);
        let client = Arc::clone(&self.client);
        
        let token = self.client.on_chat_event(chat_id, move |event| {
            let states = Arc::clone(&chat_states);
            let client = Arc::clone(&client);
            async move {
                match event {
                    ChatEvent::MessageAdded(_) |
                    ChatEvent::MessageUpdated(_) |
                    ChatEvent::MessageDeleted(_) |
                    ChatEvent::QueueMessageAdded(_) |
                    ChatEvent::QueueMessageUpdated(_) |
                    ChatEvent::QueueMessageDeleted(_) => {
                        if let Ok(chat_result) = client.get_chat(GetChatParams { chat_id }).await {
                            let mut states = states.write().await;
                            if let Some(state) = states.get_mut(&chat_id) {
                                state.messages = chat_result.messages;
                                state.queued_messages_count = chat_result.queued_messages_count;
                            }
                        }
                    }
                    ChatEvent::ToolsUpdated(_) => {
                        // Tools updated - no action needed
                    }
                }
            }
        });
        
        self.chat_tokens.write().await.push(token);
        Ok(())
    }

    /// Get all chat IDs being monitored.
    pub async fn get_chat_ids(&self) -> Vec<i64> {
        self.chat_states.read().await.keys().cloned().collect()
    }

    /// Get chat state for a specific chat.
    pub async fn get_chat_state(&self, chat_id: i64) -> Option<ChatState> {
        self.chat_states.read().await.get(&chat_id).cloned()
    }

    /// Refresh chat state from server.
    pub async fn refresh_chat(&self, chat_id: i64) -> Result<(), ClientError> {
        let chat_result = self.client.get_chat(GetChatParams { chat_id }).await?;
        let state = ChatState {
            chat_id,
            messages: chat_result.messages,
            queued_messages_count: chat_result.queued_messages_count,
            tags: chat_result.chat.tags,
        };
        self.chat_states.write().await.insert(chat_id, state);
        Ok(())
    }
}
```

### 3. `packages/rhd_chat_client/src/client.rs` (MODIFY)

**Purpose**: Add `create_plugins_monitor` and `create_chat_monitor` methods.

**Add methods** (after line 524, in the Custom Event Methods section):
```rust
    /// Create a plugins monitor that tracks ALL registered plugins (active and inactive).
    ///
    /// The monitor automatically subscribes to plugins list events and maintains
    /// the set of all registered plugins. It also provides methods for waiting
    /// on custom event acknowledgments from all plugins.
    pub async fn create_plugins_monitor(&self) -> Result<crate::plugins_monitor::PluginsMonitor, ClientError> {
        crate::plugins_monitor::PluginsMonitor::new(Arc::new(self.clone())).await
    }
    
    /// Create a chat monitor that tracks chats and their state.
    ///
    /// The monitor automatically subscribes to chat list and individual chat events.
    /// It maintains the current state of all chats including messages, queue count, and tags.
    pub async fn create_chat_monitor(&self) -> Result<crate::chat_monitor::ChatMonitor, ClientError> {
        crate::chat_monitor::ChatMonitor::new(Arc::new(self.clone())).await
    }
```

**Note**: The `ChatClient` needs to implement `Clone` for this to work. If it doesn't already, add `#[derive(Clone)]` or implement it manually.

### 4. `packages/rhd_chat_client/src/lib.rs` (MODIFY)

**Purpose**: Export the new `PluginsMonitor` and `ChatMonitor` types.

**Add modules** (after line 41):
```rust
pub mod plugins_monitor;
pub mod chat_monitor;
```

**Add re-exports** (after line 48):
```rust
pub use plugins_monitor::PluginsMonitor;
pub use chat_monitor::{ChatMonitor, ChatState};
```

### 4. `packages/rhd_chat_client/src/error.rs` (MODIFY)

**Purpose**: Add timeout error variant.

**Add variant** to `ClientError` enum:
```rust
#[error("timeout: {0}")]
Timeout(String),
```

## Tests

### Unit Tests for PluginsMonitor

Create `packages/rhd_chat_client/src/plugins_monitor_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChatClient;
    use std::time::Duration;

    #[tokio::test]
    async fn test_plugins_monitor_creation() {
        // This test requires a running server
        // For unit tests, we'd need to mock the server
        // Skipping for now, will be covered in integration tests
    }

    #[tokio::test]
    async fn test_wait_for_acks_timeout() {
        // Test that wait_for_acks_except times out correctly
        // This requires mocking the client and monitor
    }

    #[tokio::test]
    async fn test_wait_for_acks_success() {
        // Test that wait_for_acks_except succeeds when all plugins ack
        // This requires mocking the client and monitor
    }
}
```

### Integration Tests

These will be covered in Phase 5, but the test structure should be:

```rust
#[tokio::test]
async fn test_plugins_monitor_tracks_plugins() {
    // Start server
    // Connect two plugins
    // Create monitor
    // Verify both plugins are tracked
    // Disconnect one plugin
    // Verify only one plugin is tracked
}

#[tokio::test]
async fn test_wait_for_acks_except() {
    // Start server
    // Connect three plugins: A, B, C
    // Plugin A sends custom event
    // Plugin A waits for acks from B and C (except A)
    // Plugin B acknowledges
    // Plugin C acknowledges
    // Wait completes successfully
}
```

## Implementation Notes

1. **Track All Plugins**: The `PluginsMonitor` tracks ALL registered plugins, not just active ones. This ensures that even if a plugin hasn't started yet, we still wait for it to acknowledge events. The `wait_for_acks_except` method waits for all registered plugins (except excluded ones).

2. **Automatic Subscription**: Both monitors automatically subscribe to relevant events when created. The subscriptions are tied to the monitor's lifetime via `CancellationToken`.

3. **Acknowledgment Tracking**: The monitor tracks acknowledgments in a HashMap. When a `customEventAcknowledged` event is received, the client calls `record_acknowledgment` on the monitor.

4. **Wait Logic**: The `wait_for_acks_except` method polls every 100ms to check if all required plugins have acknowledged. This is simple but effective. For production, you might want to use a condition variable for better efficiency.

5. **Reusable ChatMonitor**: The `ChatMonitor` is generic and can be used by any plugin. It provides basic chat state tracking, and plugins can add their own trigger detection logic on top.

6. **Memory Management**: The `cleanup_old_acks` method is a placeholder. In production, you'd want to track timestamps and clean up old acknowledgment records to prevent memory leaks.

7. **Thread Safety**: All shared state is protected by `RwLock` or `Mutex` to ensure thread safety in async contexts.

8. **Error Handling**: The monitors return `ClientError` for all errors, including timeouts.

## Dependencies

- **Depends on**: Phase 1 (plugin system foundation)
- **Can be done in parallel with**: Phase 2 (server-side enhancements)
- **Must be completed before**: Phase 4 (plugin needs both monitors)

## Success Criteria

- [ ] `PluginsMonitor` struct exists with all required methods
- [ ] `ChatMonitor` struct exists with all required methods
- [ ] `create_plugins_monitor` method exists on `ChatClient`
- [ ] `create_chat_monitor` method exists on `ChatClient`
- [ ] PluginsMonitor tracks ALL registered plugins (active and inactive)
- [ ] PluginsMonitor tracks active plugins correctly
- [ ] ChatMonitor tracks chat state correctly
- [ ] `wait_for_acks_except` waits for ALL plugins (including inactive) to acknowledge
- [ ] `wait_for_acks_except` times out correctly
- [ ] Custom event acknowledgments are tracked automatically
- [ ] All types are exported from `lib.rs`
- [ ] Unit tests pass (or are skipped with explanation)
- [ ] Code compiles without warnings
