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
    /// Cancellation token for the custom event acknowledgment subscription.
    _ack_token: CancellationToken,
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

        // Subscribe to custom event acknowledgments
        let acknowledgments_clone = Arc::clone(&acknowledgments);
        let ack_token = client.on_custom_event_acknowledged(move |event| {
            let acks = Arc::clone(&acknowledgments_clone);
            async move {
                let mut acks_guard = acks.write().await;
                acks_guard
                    .entry(event.event_id)
                    .or_insert_with(HashSet::new)
                    .insert(event.acknowledging_plugin_id);
            }
        });

        Ok(Self {
            all_plugins,
            acknowledgments,
            _plugins_list_token: token,
            _ack_token: ack_token,
        })
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
    /// * `Err(ClientError::Timeout)` if timeout expired, with list of plugins that didn't acknowledge
    pub async fn wait_for_acks_except(
        &self,
        event_id: &str,
        except_plugin_ids: &[&str],
        timeout_duration: Duration,
    ) -> Result<(), ClientError> {
        let except_set: HashSet<String> = except_plugin_ids.iter().map(|s| s.to_string()).collect();

        let result = timeout(timeout_duration, async {
            loop {
                let (all_acked, pending_plugins) = {
                    let all_plugins = self.all_plugins.read().await;
                    let acks = self.acknowledgments.read().await;
                    let event_acks = acks.get(event_id).cloned().unwrap_or_default();

                    // Check if ALL registered plugins (except excluded ones) have acknowledged
                    let pending: Vec<String> = all_plugins.keys()
                        .filter(|id| !except_set.contains(*id) && !event_acks.contains(*id))
                        .cloned()
                        .collect();
                    
                    let all_acked = pending.is_empty();

                    if !all_acked {
                        tracing::debug!(
                            event_id = %event_id,
                            pending_plugins = ?pending,
                            "waiting for plugin acknowledgments"
                        );
                    }

                    (all_acked, pending)
                };

                if all_acked {
                    tracing::debug!(event_id = %event_id, "all required plugins acknowledged");
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
            Err(_) => {
                // Get the list of plugins that didn't acknowledge
                let all_plugins = self.all_plugins.read().await;
                let acks = self.acknowledgments.read().await;
                let event_acks = acks.get(event_id).cloned().unwrap_or_default();
                
                let pending_plugins: Vec<String> = all_plugins.keys()
                    .filter(|id| !except_set.contains(*id) && !event_acks.contains(*id))
                    .cloned()
                    .collect();
                
                Err(ClientError::Timeout(format!(
                    "Timeout waiting for plugin acknowledgments. Plugins that did not acknowledge: {:?}",
                    pending_plugins
                )))
            }
        }
    }

}
