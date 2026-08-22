//! Event types for the chat WebSocket protocol.
//!
//! Each event type has its own file containing a `Data` struct.

pub mod chat_created;
pub mod chat_deleted;
pub mod chat_updated;
pub mod custom_event;
pub mod custom_event_acknowledged;
pub mod message_added;
pub mod message_deleted;
pub mod message_updated;
pub mod plugin_registered;
pub mod plugin_removed;
pub mod plugin_updated;
pub mod queue_message_added;
pub mod queue_message_deleted;
pub mod queue_message_updated;
pub mod tools_updated;

pub use chat_created::*;
pub use chat_deleted::*;
pub use chat_updated::*;
pub use custom_event::*;
pub use custom_event_acknowledged::*;
pub use message_added::*;
pub use message_deleted::*;
pub use message_updated::*;
pub use plugin_registered::*;
pub use plugin_removed::*;
pub use plugin_updated::*;
pub use queue_message_added::*;
pub use queue_message_deleted::*;
pub use queue_message_updated::*;
pub use tools_updated::*;
