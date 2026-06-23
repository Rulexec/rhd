use std::collections::HashMap;
use std::sync::Arc;

use rhd_mcp_client::client::McpClient;
use rhd_mcp_client::McpConfig;
use tokio::sync::Mutex;

pub struct McpServerCache {
    cache: Mutex<HashMap<String, Arc<McpClient>>>,
}

impl McpServerCache {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
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

    pub async fn shutdown(&self) {
        let mut cache = self.cache.lock().await;
        for (_, client) in cache.drain() {
            if let Err(e) = client.kill().await {
                eprintln!("Failed to kill MCP server: {}", e);
            }
        }
    }
}
