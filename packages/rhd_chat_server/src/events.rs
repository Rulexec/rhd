//! Event broadcasting helpers.

use rhd_chat_api::common::{ChatSummary, Message};
use rhd_chat_api::events::{
    ChatCreatedData, ChatDeletedData, ChatUpdatedData, MessageAddedData, MessageDeletedData,
    MessageUpdatedData, QueueMessageAddedData, QueueMessageDeletedData, QueueMessageUpdatedData,
    ToolsUpdatedData,
};
use rhd_chat_api::protocol::Event;
use rhd_chat_api::tools::ToolInfo;

/// Create a `chatCreated` event.
pub fn chat_created_event(chat: ChatSummary, chat_version: i64) -> Event {
    let data = ChatCreatedData { chat, chat_version };
    Event::new("chatCreated", serde_json::to_value(data).unwrap())
}

/// Create a `chatUpdated` event.
pub fn chat_updated_event(chat: ChatSummary, chat_version: i64) -> Event {
    let data = ChatUpdatedData { chat, chat_version };
    Event::new("chatUpdated", serde_json::to_value(data).unwrap())
}

/// Create a `chatDeleted` event.
pub fn chat_deleted_event(chat_id: i64, chat_version: i64) -> Event {
    let data = ChatDeletedData { chat_id, chat_version };
    Event::new("chatDeleted", serde_json::to_value(data).unwrap())
}

/// Create a `messageAdded` event.
pub fn message_added_event(chat_id: i64, message: Message, chat_version: i64) -> Event {
    let data = MessageAddedData { chat_id, message, chat_version };
    Event::new("messageAdded", serde_json::to_value(data).unwrap())
}

/// Create a `messageUpdated` event.
pub fn message_updated_event(chat_id: i64, message: Message, chat_version: i64) -> Event {
    let data = MessageUpdatedData { chat_id, message, chat_version };
    Event::new("messageUpdated", serde_json::to_value(data).unwrap())
}

/// Create a `messageDeleted` event.
pub fn message_deleted_event(chat_id: i64, message_id: i64, chat_version: i64) -> Event {
    let data = MessageDeletedData { chat_id, message_id, chat_version };
    Event::new("messageDeleted", serde_json::to_value(data).unwrap())
}

/// Create a `queueMessageAdded` event.
pub fn queue_message_added_event(chat_id: i64, message: Message, chat_version: i64) -> Event {
    let data = QueueMessageAddedData { chat_id, message, chat_version };
    Event::new("queueMessageAdded", serde_json::to_value(data).unwrap())
}

/// Create a `queueMessageUpdated` event.
pub fn queue_message_updated_event(chat_id: i64, message: Message, chat_version: i64) -> Event {
    let data = QueueMessageUpdatedData { chat_id, message, chat_version };
    Event::new("queueMessageUpdated", serde_json::to_value(data).unwrap())
}

/// Create a `queueMessageDeleted` event.
pub fn queue_message_deleted_event(chat_id: i64, message_id: i64, chat_version: i64) -> Event {
    let data = QueueMessageDeletedData { chat_id, message_id, chat_version };
    Event::new("queueMessageDeleted", serde_json::to_value(data).unwrap())
}

/// Create a `toolsUpdated` event.
pub fn tools_updated_event(chat_id: i64, tools: Vec<ToolInfo>, chat_version: i64) -> Event {
    let data = ToolsUpdatedData { chat_id, tools, chat_version };
    Event::new("toolsUpdated", serde_json::to_value(data).unwrap())
}
