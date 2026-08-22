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

    /// Path to SQLite database
    #[arg(long, default_value = "./rhd_db/chats.db")]
    pub db_path: String,
}

impl Config {
    /// Get the socket address string.
    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
