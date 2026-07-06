use std::collections::HashMap;
use std::sync::Arc;

use rhd_mcp_client::client::McpClient;
use rhd_mcp_client::McpConfig;
use tokio::sync::Mutex;

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

        let mut cache = self.cache.lock().await;
        
        if let Some(client) = cache.get(&cache_key) {
            return Ok(client.clone());
        }

        let client = McpClient::connect(
            config.cmd.as_deref().unwrap_or(""),
            &config.args,
            config.cwd.as_deref(),
            &config.env,
        )
        .await?;

        let client = Arc::new(client);
        cache.insert(cache_key, client.clone());
        Ok(client)
    }

    #[allow(dead_code)]
    pub async fn shutdown(&self) {
        let mut cache = self.cache.lock().await;
        for (_, client) in cache.drain() {
            if let Err(e) = client.kill().await {
                eprintln!("Failed to kill MCP server: {}", e);
            }
        }
    }

    pub async fn stop_specific(&self, keys_to_stop: &[String]) -> usize {
        let mut cache = self.cache.lock().await;
        let mut stopped = 0;
        for key in keys_to_stop {
            if let Some(client) = cache.remove(key) {
                if let Some(pid) = client.pid().await {
                    eprintln!("stopping MCP server '{}' (PID: {})", key, pid);
                } else {
                    eprintln!("stopping MCP server '{}'", key);
                }
                if let Err(e) = client.kill().await {
                    eprintln!("failed to kill MCP server '{}': {}", key, e);
                }
                stopped += 1;
            }
        }
        stopped
    }

    pub async fn restart_specific(&self, keys_to_restart: &[String]) -> usize {
        self.stop_specific(keys_to_restart).await
    }
}
