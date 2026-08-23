mod chats;
mod custom_events;
mod messages;
mod messages_queue;
mod plugins;
mod schema;
mod tags;
mod tools;

#[cfg(test)]
mod tests;

pub use custom_events::CustomEventInfo;
pub use plugins::PluginInfo;
pub use tools::{ToolDefinition, FunctionDefinition};

use rusqlite::Connection;
use std::sync::Mutex;

use crate::DbResult;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatInfo {
    pub id: i64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub active_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: i64,
    pub chat_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub model: Option<String>,
    pub thinking_content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

pub struct ChatDb {
    conn: Mutex<Connection>,
}

impl ChatDb {
    pub fn new(path: &str) -> DbResult<Self> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        {
            let conn_guard = db.conn.lock().map_err(|e| crate::DbError::InitializationError(e.to_string()))?;
            schema::init(&conn_guard)?;
        }
        Ok(db)
    }

    pub fn create_chat(&self, title: &str) -> DbResult<i64> {
        chats::create_chat(&self.conn, title)
    }

    pub fn list_chats(&self) -> DbResult<Vec<ChatInfo>> {
        chats::list_chats(&self.conn)
    }

    pub fn get_chat(&self, id: i64) -> DbResult<Option<ChatInfo>> {
        chats::get_chat(&self.conn, id)
    }

    pub fn delete_chat(&self, id: i64) -> DbResult<()> {
        chats::delete_chat(&self.conn, id)
    }

    pub fn update_chat_title(&self, id: i64, title: &str) -> DbResult<()> {
        chats::update_chat_title(&self.conn, id, title)
    }

    pub fn touch_chat(&self, id: i64) -> DbResult<()> {
        chats::touch_chat(&self.conn, id)
    }

    pub fn add_message(
        &self,
        chat_id: i64,
        role: &str,
        content: &str,
        model: Option<&str>,
        thinking_content: Option<&str>,
    ) -> DbResult<i64> {
        messages::add_message(&self.conn, chat_id, role, content, model, thinking_content)
    }

    pub fn get_messages(&self, chat_id: i64) -> DbResult<Vec<Message>> {
        messages::get_messages(&self.conn, chat_id)
    }

    pub fn get_message(&self, message_id: i64) -> DbResult<Option<Message>> {
        messages::get_message(&self.conn, message_id)
    }

    pub fn update_message(&self, message_id: i64, content: &str) -> DbResult<()> {
        messages::update_message(&self.conn, message_id, content)
    }

    pub fn delete_message(&self, message_id: i64) -> DbResult<()> {
        messages::delete_message(&self.conn, message_id)
    }

    // Tag operations
    pub fn get_chat_tags(&self, chat_id: i64) -> DbResult<Vec<String>> {
        tags::get_chat_tags(&self.conn, chat_id)
    }

    pub fn get_message_tags(&self, message_id: i64) -> DbResult<Vec<String>> {
        tags::get_message_tags(&self.conn, message_id)
    }

    pub fn set_chat_tags(&self, chat_id: i64, tags: &[String]) -> DbResult<()> {
        tags::set_chat_tags(&self.conn, chat_id, tags)
    }

    pub fn set_message_tags(&self, message_id: i64, tags: &[String]) -> DbResult<()> {
        tags::set_message_tags(&self.conn, message_id, tags)
    }

    pub fn add_chat_tags(&self, chat_id: i64, tags: &[String]) -> DbResult<()> {
        tags::add_chat_tags(&self.conn, chat_id, tags)
    }

    pub fn add_message_tags(&self, message_id: i64, tags: &[String]) -> DbResult<()> {
        tags::add_message_tags(&self.conn, message_id, tags)
    }

    pub fn remove_chat_tags(&self, chat_id: i64, tags: &[String]) -> DbResult<()> {
        tags::remove_chat_tags(&self.conn, chat_id, tags)
    }

    pub fn remove_message_tags(&self, message_id: i64, tags: &[String]) -> DbResult<()> {
        tags::remove_message_tags(&self.conn, message_id, tags)
    }

    // Plugin operations
    pub fn register_plugin(&self, plugin_id: &str) -> DbResult<()> {
        plugins::register_plugin(&self.conn, plugin_id)
    }

    pub fn deactivate_plugin(&self, plugin_id: &str) -> DbResult<()> {
        plugins::deactivate_plugin(&self.conn, plugin_id)
    }

    pub fn remove_plugin(&self, plugin_id: &str) -> DbResult<()> {
        plugins::remove_plugin(&self.conn, plugin_id)
    }

    pub fn get_plugins(&self) -> DbResult<Vec<PluginInfo>> {
        plugins::get_plugins(&self.conn)
    }

    // Custom event operations
    pub fn create_custom_event(
        &self,
        event_id: &str,
        event_name: &str,
        sender_plugin_id: Option<&str>,
        additional: Option<&str>,
    ) -> DbResult<()> {
        custom_events::create_custom_event(&self.conn, event_id, event_name, sender_plugin_id, additional)
    }

    pub fn get_custom_event(&self, event_id: &str) -> DbResult<Option<CustomEventInfo>> {
        custom_events::get_custom_event(&self.conn, event_id)
    }

    pub fn ack_custom_event(&self, event_id: &str, plugin_id: &str) -> DbResult<()> {
        custom_events::ack_custom_event(&self.conn, event_id, plugin_id)
    }

    pub fn has_plugin_acked(&self, event_id: &str, plugin_id: &str) -> DbResult<bool> {
        custom_events::has_plugin_acked(&self.conn, event_id, plugin_id)
    }

    pub fn get_pending_events_for_plugin(&self, plugin_id: &str) -> DbResult<Vec<CustomEventInfo>> {
        custom_events::get_pending_events_for_plugin(&self.conn, plugin_id)
    }

    pub fn delete_custom_event(&self, event_id: &str) -> DbResult<()> {
        custom_events::delete_custom_event(&self.conn, event_id)
    }

    // Queue message operations
    pub fn add_queue_message(
        &self,
        chat_id: i64,
        role: &str,
        content: &str,
        model: Option<&str>,
        thinking_content: Option<&str>,
    ) -> DbResult<i64> {
        messages_queue::add_queue_message(&self.conn, chat_id, role, content, model, thinking_content)
    }

    pub fn get_queue_messages(&self, chat_id: i64) -> DbResult<Vec<Message>> {
        messages_queue::get_queue_messages(&self.conn, chat_id)
    }

    pub fn get_queue_message(&self, message_id: i64) -> DbResult<Option<Message>> {
        messages_queue::get_queue_message(&self.conn, message_id)
    }

    pub fn update_queue_message(&self, message_id: i64, content: &str) -> DbResult<()> {
        messages_queue::update_queue_message(&self.conn, message_id, content)
    }

    pub fn insert_queue_message(&self, message: &Message) -> DbResult<Message> {
        messages_queue::insert_queue_message(&self.conn, message)
    }

    pub fn update_queue_message_full(&self, message: &Message) -> DbResult<Message> {
        messages_queue::update_queue_message_full(&self.conn, message)
    }

    pub fn delete_queue_message(&self, message_id: i64) -> DbResult<()> {
        messages_queue::delete_queue_message(&self.conn, message_id)
    }

    pub fn delete_all_queue_messages(&self, chat_id: i64) -> DbResult<()> {
        messages_queue::delete_all_queue_messages(&self.conn, chat_id)
    }

    pub fn count_queue_messages(&self, chat_id: i64) -> DbResult<i64> {
        messages_queue::count_queue_messages(&self.conn, chat_id)
    }

    // Queue message tag operations
    pub fn get_queue_message_tags(&self, message_id: i64) -> DbResult<Vec<String>> {
        tags::get_queue_message_tags(&self.conn, message_id)
    }

    pub fn set_queue_message_tags(&self, message_id: i64, tags: &[String]) -> DbResult<()> {
        tags::set_queue_message_tags(&self.conn, message_id, tags)
    }

    pub fn add_queue_message_tags(&self, message_id: i64, tags: &[String]) -> DbResult<()> {
        tags::add_queue_message_tags(&self.conn, message_id, tags)
    }

    pub fn remove_queue_message_tags(&self, message_id: i64, tags: &[String]) -> DbResult<()> {
        tags::remove_queue_message_tags(&self.conn, message_id, tags)
    }

    // Tool operations
    pub fn add_chat_tools(
        &self,
        chat_id: i64,
        plugin_id: &str,
        tools: &[ToolDefinition],
    ) -> DbResult<()> {
        tools::add_chat_tools(&self.conn, chat_id, plugin_id, tools)
    }

    pub fn remove_chat_tools(
        &self,
        chat_id: i64,
        plugin_id: &str,
        tool_names: &[String],
    ) -> DbResult<()> {
        tools::remove_chat_tools(&self.conn, chat_id, plugin_id, tool_names)
    }

    pub fn get_chat_tools(&self, chat_id: i64) -> DbResult<Vec<(String, ToolDefinition)>> {
        tools::get_chat_tools(&self.conn, chat_id)
    }

    pub fn get_chat_tools_by_plugin(
        &self,
        chat_id: i64,
        plugin_id: &str,
    ) -> DbResult<Vec<ToolDefinition>> {
        tools::get_chat_tools_by_plugin(&self.conn, chat_id, plugin_id)
    }
}
