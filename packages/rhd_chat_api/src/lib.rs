//! RHD Chat API — API data types for the chat WebSocket protocol.
//!
//! This library contains all API types (requests, responses, events) for the
//! chat WebSocket protocol. It is reusable by Rust clients connecting to the
//! chat server.
//!
//! # Architecture
//!
//! - **Shared types** in [`common`] — `Chat`, `Message`, `ChatSummary`, `PluginSummary`, `PendingEvent`
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
pub mod tools;

// Re-export commonly used types at the crate root for convenience
pub use common::{Chat, ChatSummary, Message, PendingEvent, PluginSummary};
pub use error::{ErrorCode, ErrorResponse};
pub use events::{
    AssistantMessageWithToolCallsData, ChatCreatedData, ChatDeletedData, ChatUpdatedData,
    CustomEventAcknowledgedData, CustomEventData, MessageAddedData, MessageDeletedData,
    MessageUpdatedData, PluginRegisteredData, PluginRemovedData, PluginUpdatedData,
    QueueMessageAddedData, QueueMessageDeletedData, QueueMessageUpdatedData, StreamChunkData,
    StreamFinishedData, ToolsUpdatedData,
};
pub use methods::{
    AckCustomEventParams, AckCustomEventResult, AddMessageParams, AddMessageResult,
    AddQueueMessageParams, AddQueueMessageResult, AddToolsParams, AddToolsResult,
    CreateChatParams, CreateChatResult, DeleteChatParams, DeleteChatResult, DeleteMessageParams,
    DeleteMessageResult, DeleteQueueMessageParams, DeleteQueueMessageResult, GetChatParams,
    GetChatResult, GetMessagesParams, GetMessagesResult, GetPendingAcksParams, GetPendingAcksResult,
    GetPluginsParams, GetPluginsResult, GetQueueMessagesParams, GetQueueMessagesResult,
    GetToolsParams, GetToolsResult, ListChatsParams, ListChatsResult, RegisterPluginParams,
    RegisterPluginResult, RemovePluginParams, RemovePluginResult, RemoveToolsParams,
    RemoveToolsResult, SendCustomEventParams, SendCustomEventResult, StreamFinishParams,
    StreamFinishResult, StreamPushParams, StreamPushResult, StreamSubscribeParams,
    StreamSubscribeResult, StreamToolCallDelta, SubscribeChatParams, SubscribeChatResult,
    SubscribeChatsListParams, SubscribeChatsListResult, SubscribePluginsListParams,
    SubscribePluginsListResult, UnsubscribeChatParams, UnsubscribeChatResult,
    UnsubscribeChatsListParams, UnsubscribeChatsListResult, UnsubscribePluginsListParams,
    UnsubscribePluginsListResult, UpdateChatParams, UpdateChatResult, UpdateMessageParams,
    UpdateMessageResult, UpdateQueueMessageParams, UpdateQueueMessageResult,
    UpdateToolCallTagsParams, UpdateToolCallTagsResult,
};
pub use protocol::{Event, Request, Response};
pub use tools::{FunctionCall, FunctionDefinition, ToolCall, ToolDefinition, ToolInfo};
