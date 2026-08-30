//! Server configuration.

use clap::Parser;
use std::format;

/// RHD Chat Server configuration.
#[derive(Parser, Debug, Clone)]
#[command(name = "rhd_chat_server")]
#[command(about = "WebSocket server for chat storage and management")]
pub struct Config {
    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to listen on
    #[arg(long, default_value = "8080")]
    pub port: u16,

    /// Path to folder for SQLite database (chats.db will be created inside)
    #[arg(long, default_value = "./rhd_db")]
    pub db_path: String,

    /// Clear all pending custom event acknowledgments before starting server
    #[arg(long)]
    pub clear_pending_acks: bool,
}

impl Config {
    /// Get the socket address string.
    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
