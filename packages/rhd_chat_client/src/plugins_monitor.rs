//! Plugins monitor implementation for tracking registered plugins and acknowledgments.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::time::timeout;

use rhd_chat_api::GetPluginsParams;

use crate::client::ChatClient;
use crate::error::ClientError;
use crate::event_stream::{CancellationToken, PluginsListEvent};

/// Information about a registered plugin.
#[derive(Clone)]
struct PluginInfo {
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
                plugins.insert(
                    plugin.plugin_id.clone(),
                    PluginInfo {
                        is_active: plugin.is_active,
                    },
                );
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
                        plugins.insert(
                            data.plugin_id.clone(),
                            PluginInfo {
                                is_active: data.is_active,
                            },
                        );
                    }
                    PluginsListEvent::PluginUpdated(data) => {
                        let mut plugins = plugins.write().await;
                        if let Some(info) = plugins.get_mut(&data.plugin_id) {
                            info.is_active = data.is_active;
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
        self.all_plugins
            .read()
            .await
            .iter()
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
        self.all_plugins
            .read()
            .await
            .get(plugin_id)
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
                let all_acked = {
                    let all_plugins = self.all_plugins.read().await;
                    let acks = self.acknowledgments.read().await;
                    let event_acks = acks.get(event_id).cloned().unwrap_or_default();

                    // Check if ALL registered plugins (except excluded ones) have acknowledged
                    all_plugins.keys().all(|plugin_id| {
                        except_set.contains(plugin_id) || event_acks.contains(plugin_id)
                    })
                };

                if all_acked {
                    return Ok(());
                }

                // Wait a bit before checking again
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(ClientError::Timeout(
                "Timeout waiting for plugin acknowledgments".to_string(),
            )),
        }
    }

    /// Clean up old acknowledgment records.
    ///
    /// This should be called periodically to prevent memory leaks.
    pub async fn cleanup_old_acks(&self, _max_age: Duration) {
        // Note: This requires tracking timestamps for acknowledgments
        // For now, we'll skip this implementation as it's not critical
        // In production, you'd want to add timestamps and clean up old entries
    }
}
