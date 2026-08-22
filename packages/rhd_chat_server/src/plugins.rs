//! Plugin registry for tracking plugin-to-connection mappings.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tokio::sync::RwLock;

use rhd_db::ChatDb;

use crate::error::ServerError;
use crate::subscriptions::ConnectionId;

/// Plugin registry that tracks plugin-to-connection mappings.
#[derive(Default)]
pub struct PluginRegistry {
    /// Map from plugin_id to connection_id.
    plugin_to_connection: HashMap<String, ConnectionId>,
    /// Map from connection_id to set of plugin_ids.
    connection_to_plugins: HashMap<ConnectionId, HashSet<String>>,
}

impl PluginRegistry {
    /// Create a new plugin registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a plugin for a connection.
    /// Returns true if this is a new plugin, false if it was re-registered.
    pub fn register_plugin(&mut self, plugin_id: &str, connection_id: &str) -> bool {
        let is_new = !self.plugin_to_connection.contains_key(plugin_id);

        // Remove from old connection if re-registering
        if let Some(old_connection_id) = self.plugin_to_connection.get(plugin_id) {
            if old_connection_id != connection_id {
                if let Some(plugins) = self.connection_to_plugins.get_mut(old_connection_id) {
                    plugins.remove(plugin_id);
                }
            }
        }

        self.plugin_to_connection.insert(plugin_id.to_string(), connection_id.to_string());
        self.connection_to_plugins
            .entry(connection_id.to_string())
            .or_insert_with(HashSet::new)
            .insert(plugin_id.to_string());

        is_new
    }

    /// Get the connection ID for a plugin.
    pub fn get_connection_for_plugin(&self, plugin_id: &str) -> Option<&str> {
        self.plugin_to_connection.get(plugin_id).map(|s| s.as_str())
    }

    /// Get all plugin IDs for a connection.
    pub fn get_plugins_for_connection(&self, connection_id: &str) -> Vec<String> {
        self.connection_to_plugins
            .get(connection_id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Remove all plugins for a connection and return their IDs.
    /// Called when a connection disconnects.
    pub fn remove_plugins_for_connection(&mut self, connection_id: &str) -> Vec<String> {
        let plugin_ids = self.connection_to_plugins.remove(connection_id).unwrap_or_default();
        for plugin_id in &plugin_ids {
            self.plugin_to_connection.remove(plugin_id);
        }
        plugin_ids.into_iter().collect()
    }

    /// Remove a specific plugin.
    pub fn remove_plugin(&mut self, plugin_id: &str) -> Option<String> {
        if let Some(connection_id) = self.plugin_to_connection.remove(plugin_id) {
            if let Some(plugins) = self.connection_to_plugins.get_mut(&connection_id) {
                plugins.remove(plugin_id);
            }
            Some(connection_id)
        } else {
            None
        }
    }

}

/// Thread-safe wrapper for plugin registry.
pub type SharedPluginRegistry = Arc<RwLock<PluginRegistry>>;

/// Create a new shared plugin registry.
pub fn new_shared_plugin_registry() -> SharedPluginRegistry {
    Arc::new(RwLock::new(PluginRegistry::new()))
}

/// Deactivate all plugins for a connection when it disconnects.
pub async fn deactivate_plugins_on_disconnect(
    db: &ChatDb,
    plugin_registry: &SharedPluginRegistry,
    connection_id: &str,
) -> Result<Vec<String>, ServerError> {
    let mut registry = plugin_registry.write().await;
    let plugin_ids = registry.remove_plugins_for_connection(connection_id);

    for plugin_id in &plugin_ids {
        db.deactivate_plugin(plugin_id)?;
    }

    Ok(plugin_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registry_register() {
        let mut registry = PluginRegistry::new();
        
        let is_new = registry.register_plugin("plugin-1", "conn-1");
        assert!(is_new);
        
        assert_eq!(registry.get_connection_for_plugin("plugin-1"), Some("conn-1"));
        assert_eq!(registry.get_plugins_for_connection("conn-1"), vec!["plugin-1".to_string()]);
    }

    #[test]
    fn test_plugin_registry_re_register() {
        let mut registry = PluginRegistry::new();
        
        registry.register_plugin("plugin-1", "conn-1");
        let is_new = registry.register_plugin("plugin-1", "conn-2");
        
        assert!(!is_new);
        assert_eq!(registry.get_connection_for_plugin("plugin-1"), Some("conn-2"));
    }

    #[test]
    fn test_plugin_registry_remove() {
        let mut registry = PluginRegistry::new();
        
        registry.register_plugin("plugin-1", "conn-1");
        registry.remove_plugin("plugin-1");
        
        assert_eq!(registry.get_connection_for_plugin("plugin-1"), None);
        assert!(registry.get_plugins_for_connection("conn-1").is_empty());
    }

    #[test]
    fn test_plugin_registry_remove_plugins_for_connection() {
        let mut registry = PluginRegistry::new();
        
        registry.register_plugin("plugin-1", "conn-1");
        registry.register_plugin("plugin-2", "conn-1");
        
        let removed = registry.remove_plugins_for_connection("conn-1");
        assert_eq!(removed.len(), 2);
        assert!(removed.contains(&"plugin-1".to_string()));
        assert!(removed.contains(&"plugin-2".to_string()));
        
        assert_eq!(registry.get_connection_for_plugin("plugin-1"), None);
        assert_eq!(registry.get_connection_for_plugin("plugin-2"), None);
    }

    #[test]
    fn test_plugin_registry_multiple_plugins_per_connection() {
        let mut registry = PluginRegistry::new();
        
        registry.register_plugin("plugin-1", "conn-1");
        registry.register_plugin("plugin-2", "conn-1");
        
        let plugins = registry.get_plugins_for_connection("conn-1");
        assert_eq!(plugins.len(), 2);
    }
}
