use std::collections::HashMap;
use std::sync::Arc;

use rhd_mcp_client::client::McpClient;
use rhd_mcp_client::McpConfig;
use tokio::sync::Mutex;
use tracing::{info, error};

#[derive(Clone)]
pub struct McpServerCache {
    cache: Arc<Mutex<HashMap<String, Arc<McpClient>>>>,
}

impl McpServerCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[tracing::instrument(level = "debug", skip(self, config), fields(cmd = ?config.cmd, args = ?config.args, cwd = ?config.cwd))]
    pub async fn get_or_spawn(
        &self,
        config: &McpConfig,
    ) -> Result<Arc<McpClient>, rhd_mcp_client::McpError> {
        let cache_key = format!(
            "{}:{}:{}",
            config.cmd.as_deref().unwrap_or(""),
            config.args.join(","),
            config.cwd.as_deref().unwrap_or("")
        );

        {
            let cache = self.cache.lock().await;
            if let Some(client) = cache.get(&cache_key) {
                return Ok(client.clone());
            }
        }

        let client = McpClient::connect(
            config.cmd.as_deref().unwrap_or(""),
            &config.args,
            config.cwd.as_deref(),
            &config.env,
        )
        .await?;

        let client = Arc::new(client);
        let mut cache = self.cache.lock().await;
        let client = cache.entry(cache_key).or_insert(client).clone();
        Ok(client)
    }

    #[tracing::instrument(level = "info", skip(self, keys_to_stop), fields(keys = ?keys_to_stop))]
    pub async fn stop_specific(&self, keys_to_stop: &[String]) -> usize {
        let mut cache = self.cache.lock().await;
        let mut stopped = 0;
        for key in keys_to_stop {
            if let Some(client) = cache.remove(key) {
                if let Some(pid) = client.pid().await {
                    info!(%key, ?pid, "stopping MCP server");
                } else {
                    info!(%key, "stopping MCP server");
                }
                if let Err(e) = client.kill().await {
                    error!(%key, %e, "failed to kill MCP server");
                }
                stopped += 1;
            }
        }
        stopped
    }

    #[tracing::instrument(level = "info", skip(self, keys_to_restart), fields(keys = ?keys_to_restart))]
    pub async fn restart_specific(&self, keys_to_restart: &[String]) -> usize {
        self.stop_specific(keys_to_restart).await
    }
}
