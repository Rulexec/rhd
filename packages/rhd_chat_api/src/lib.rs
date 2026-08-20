//! RHD Chat API — API data types for the chat WebSocket protocol.
//!
//! This library contains all API types (requests, responses, events) for the
//! chat WebSocket protocol. It is reusable by Rust clients connecting to the
//! chat server.
//!
//! # Architecture
//!
//! - **Shared types** in [`common`] — `Chat`, `Message`, `ChatSummary`
//! - **Error types** in [`error`] — `ErrorCode`, `ErrorResponse`
//! - **Protocol envelope** in [`protocol`] — `Request`, `Response`, `Event`, `Message`
//! - **Method types** in [`methods`] — One file per method with `Params` and `Result` structs
//! - **Event types** in [`events`] — One file per event type with `Data` structs
//!
//! # Example
//!
//! ```rust
//! use rhd_chat_api::methods::{CreateChatParams, CreateChatResult};
//! use rhd_chat_api::events::{ChatCreatedData};
//! use rhd_chat_api::common::{Chat, Message};
//! use rhd_chat_api::error::{ErrorCode, ErrorResponse};
//! use rhd_chat_api::protocol::{Request, Response, Event};
//! ```

pub mod common;
pub mod error;
pub mod events;
pub mod methods;
pub mod protocol;

// Re-export commonly used types at the crate root for convenience
pub use common::{Chat, ChatSummary, Message};
pub use error::{ErrorCode, ErrorResponse};
pub use events::{
    ChatCreatedData, ChatDeletedData, ChatUpdatedData, MessageAddedData, MessageDeletedData,
    MessageUpdatedData,
};
pub use methods::{
    AddMessageParams, AddMessageResult, CreateChatParams, CreateChatResult, DeleteChatParams,
    DeleteChatResult, DeleteMessageParams, DeleteMessageResult, GetChatParams, GetChatResult,
    ListChatsParams, ListChatsResult, SubscribeChatParams, SubscribeChatResult,
    SubscribeChatsListParams, SubscribeChatsListResult, UnsubscribeChatParams,
    UnsubscribeChatResult, UnsubscribeChatsListParams, UnsubscribeChatsListResult,
    UpdateChatParams, UpdateChatResult, UpdateMessageParams, UpdateMessageResult,
};
pub use protocol::{Event, Request, Response};
