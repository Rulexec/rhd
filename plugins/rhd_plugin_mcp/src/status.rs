//! Per-MCP-server run status tracking and `mcpStatus:1` state reporting.

use std::collections::HashMap;

use rhd_chat_api::{StateFormat, UpdatePluginStateParams};
use rhd_chat_client::ChatClient;
use serde::Serialize;
use tokio::sync::Mutex;

use crate::mcp_pool::ServerStartupReport;

/// One entry of the `mcpStatus:1` payload.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct McpStatusEntry {
    pub id: String,
    pub name: String,
    /// "ok" | "error"
    pub status: &'static str,
    /// Present only for errored servers: startup / broken-protocol /
    /// crash-on-call message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The full `mcpStatus:1` state content.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct McpStatusPayload {
    pub mcp: Vec<McpStatusEntry>,
}

/// The state key and schema this plugin publishes.
pub const STATE_KEY: &str = "status";
pub const STATE_SCHEMA: &str = "mcpStatus:1";

/// Live status of every configured server, in config order (stable payload).
pub struct McpStatusTracker {
    order: Vec<(String, String)>, // (id, name) — fixed at construction
    statuses: Mutex<HashMap<String, ServerRunStatus>>,
    last_pushed: Mutex<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerRunStatus {
    pub ok: bool,
    pub error: Option<String>,
}

impl McpStatusTracker {
    /// Build from the resolved config order; all servers start "unknown-ok"
    /// and are immediately corrected by `record_startup`.
    pub fn new(order: Vec<(String, String)>) -> Self {
        let statuses = order
            .iter()
            .map(|(id, _name)| {
                (
                    id.clone(),
                    ServerRunStatus {
                        ok: true,
                        error: None,
                    },
                )
            })
            .collect();
        Self {
            order,
            statuses: Mutex::new(statuses),
            last_pushed: Mutex::new(None),
        }
    }

    /// Apply per-server startup outcomes from the pool.
    pub async fn record_startup(&self, reports: &[ServerStartupReport]) {
        let mut statuses = self.statuses.lock().await;
        for report in reports {
            statuses.insert(
                report.id.clone(),
                ServerRunStatus {
                    ok: report.error.is_none(),
                    error: report.error.clone(),
                },
            );
        }
    }

    pub async fn mark_error(&self, server_id: &str, message: String) {
        self.statuses.lock().await.insert(
            server_id.to_string(),
            ServerRunStatus {
                ok: false,
                error: Some(message),
            },
        );
    }

    pub async fn mark_ok(&self, server_id: &str) {
        self.statuses.lock().await.insert(
            server_id.to_string(),
            ServerRunStatus {
                ok: true,
                error: None,
            },
        );
    }

    /// Serialize the payload in config order.
    pub async fn payload_json(&self) -> String {
        let statuses = self.statuses.lock().await;
        let entries: Vec<McpStatusEntry> = self
            .order
            .iter()
            .map(|(id, name)| match statuses.get(id) {
                Some(s) if s.ok => McpStatusEntry {
                    id: id.clone(),
                    name: name.clone(),
                    status: "ok",
                    error: None,
                },
                Some(s) => McpStatusEntry {
                    id: id.clone(),
                    name: name.clone(),
                    status: "error",
                    error: s.error.clone(),
                },
                None => McpStatusEntry {
                    id: id.clone(),
                    name: name.clone(),
                    status: "error",
                    error: Some("status not recorded".to_string()),
                },
            })
            .collect();
        serde_json::to_string(&McpStatusPayload { mcp: entries }).unwrap()
    }

    /// Push state only when the payload differs from the last successful push
    /// (dedup — the server still assigns versions; this saves traffic).
    ///
    /// Lock discipline: the payload is built with the status lock released
    /// before the network await; `last_pushed` is updated under a short lock.
    pub async fn push_if_changed(&self, client: &ChatClient) -> Result<(), StatusReportError> {
        let json = self.payload_json().await;
        if self.last_pushed.lock().await.as_deref() == Some(json.as_str()) {
            return Ok(());
        }
        client
            .update_plugin_state(UpdatePluginStateParams {
                key: STATE_KEY.to_string(),
                content: json.clone(),
                format: StateFormat::Json,
                schema: STATE_SCHEMA.to_string(),
            })
            .await
            .map_err(|e| StatusReportError::Push(e.to_string()))?;
        *self.last_pushed.lock().await = Some(json);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StatusReportError {
    #[error("failed to push mcp status state: {0}")]
    Push(String),
}
