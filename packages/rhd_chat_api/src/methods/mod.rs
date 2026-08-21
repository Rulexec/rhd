//! Method types for the chat WebSocket protocol.
//!
//! Each method has its own file containing `Params` and `Result` structs.

pub mod ack_custom_event;
pub mod add_message;
pub mod create_chat;
pub mod delete_chat;
pub mod delete_message;
pub mod get_chat;
pub mod get_pending_acks;
pub mod get_plugins;
pub mod list_chats;
pub mod register_plugin;
pub mod remove_plugin;
pub mod send_custom_event;
pub mod subscribe_chat;
pub mod subscribe_chats_list;
pub mod subscribe_plugins_list;
pub mod unsubscribe_chat;
pub mod unsubscribe_chats_list;
pub mod unsubscribe_plugins_list;
pub mod update_chat;
pub mod update_message;

pub use ack_custom_event::*;
pub use add_message::*;
pub use create_chat::*;
pub use delete_chat::*;
pub use delete_message::*;
pub use get_chat::*;
pub use get_pending_acks::*;
pub use get_plugins::*;
pub use list_chats::*;
pub use register_plugin::*;
pub use remove_plugin::*;
pub use send_custom_event::*;
pub use subscribe_chat::*;
pub use subscribe_chats_list::*;
pub use subscribe_plugins_list::*;
pub use unsubscribe_chat::*;
pub use unsubscribe_chats_list::*;
pub use unsubscribe_plugins_list::*;
pub use update_chat::*;
pub use update_message::*;
