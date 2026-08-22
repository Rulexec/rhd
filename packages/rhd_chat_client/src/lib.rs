//! RHD Chat Client — WebSocket client for the chat server.
//!
//! This library provides a typed async client for connecting to the RHD chat server.
//! It depends only on `rhd_chat_api` for protocol types, making it reusable by any
//! Rust client.
//!
//! # Example
//!
//! ```rust,no_run
//! use rhd_chat_client::ChatClient;
//! use rhd_chat_api::{CreateChatParams, AddMessageParams};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Connect to the server
//!     let client = ChatClient::connect("ws://127.0.0.1:8080/").await?;
//!
//!     // Create a chat
//!     let result = client.create_chat(CreateChatParams {
//!         title: "My Chat".to_string(),
//!         tags: vec!["test".to_string()],
//!     }).await?;
//!
//!     let chat_id = result.chat_id;
//!
//!     // Add a message
//!     client.add_message(AddMessageParams {
//!         chat_id,
//!         role: "user".to_string(),
//!         content: "Hello!".to_string(),
//!         reasoning_content: None,
//!         tags: vec![],
//!     }).await?;
//!
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod error;
pub mod event_stream;
pub mod plugins_monitor;
pub mod chat_monitor;

// Re-export main types at crate root for convenience
pub use client::ChatClient;
pub use error::ClientError;
pub use event_stream::{
    ChatEvent, ChatsListEvent, CancellationToken, PluginsListEvent,
};
pub use plugins_monitor::PluginsMonitor;
pub use chat_monitor::{ChatMonitor, ChatState};
