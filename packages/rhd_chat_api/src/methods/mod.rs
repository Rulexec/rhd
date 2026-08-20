//! Method types for the chat WebSocket protocol.
//!
//! Each method has its own file containing `Params` and `Result` structs.

pub mod add_message;
pub mod create_chat;
pub mod delete_chat;
pub mod delete_message;
pub mod get_chat;
pub mod list_chats;
pub mod subscribe_chat;
pub mod subscribe_chats_list;
pub mod unsubscribe_chat;
pub mod unsubscribe_chats_list;
pub mod update_chat;
pub mod update_message;

pub use add_message::*;
pub use create_chat::*;
pub use delete_chat::*;
pub use delete_message::*;
pub use get_chat::*;
pub use list_chats::*;
pub use subscribe_chat::*;
pub use subscribe_chats_list::*;
pub use unsubscribe_chat::*;
pub use unsubscribe_chats_list::*;
pub use update_chat::*;
pub use update_message::*;
