mod chats;
mod messages;
mod projects;
mod schema;

#[cfg(test)]
mod tests;

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
    pub active_role_project: Option<String>,
    pub active_role_name: Option<String>,
    pub todo_list: Option<String>,
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

    pub fn delete_all_chats(&self) -> DbResult<()> {
        chats::delete_all_chats(&self.conn)
    }

    pub fn update_chat_title(&self, id: i64, title: &str) -> DbResult<()> {
        chats::update_chat_title(&self.conn, id, title)
    }

    pub fn update_chat_active_model(&self, id: i64, model: &str) -> DbResult<()> {
        chats::update_chat_active_model(&self.conn, id, model)
    }

    pub fn touch_chat(&self, id: i64) -> DbResult<()> {
        chats::touch_chat(&self.conn, id)
    }

    pub fn set_active_role(&self, chat_id: i64, project_name: &str, role_name: &str) -> DbResult<()> {
        chats::set_active_role(&self.conn, chat_id, project_name, role_name)
    }

    pub fn clear_active_role(&self, chat_id: i64) -> DbResult<()> {
        chats::clear_active_role(&self.conn, chat_id)
    }

    pub fn get_active_role(&self, chat_id: i64) -> DbResult<Option<(String, String)>> {
        chats::get_active_role(&self.conn, chat_id)
    }

    pub fn mark_roles_list_injected(&self, chat_id: i64) -> DbResult<()> {
        chats::mark_roles_list_injected(&self.conn, chat_id)
    }

    pub fn has_roles_list_been_injected(&self, chat_id: i64) -> DbResult<bool> {
        chats::has_roles_list_been_injected(&self.conn, chat_id)
    }

    pub fn reset_roles_list_injected(&self, chat_id: i64) -> DbResult<()> {
        chats::reset_roles_list_injected(&self.conn, chat_id)
    }

    pub fn set_role_prompt_pending(&self, chat_id: i64, pending: bool) -> DbResult<()> {
        chats::set_role_prompt_pending(&self.conn, chat_id, pending)
    }

    pub fn has_role_prompt_pending(&self, chat_id: i64) -> DbResult<bool> {
        chats::has_role_prompt_pending(&self.conn, chat_id)
    }

    pub fn set_todo_list(&self, chat_id: i64, todo_list: &str) -> DbResult<()> {
        chats::set_todo_list(&self.conn, chat_id, todo_list)
    }

    pub fn get_todo_list(&self, chat_id: i64) -> DbResult<Option<String>> {
        chats::get_todo_list(&self.conn, chat_id)
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

    pub fn truncate_messages(&self, chat_id: i64, after_message_id: i64) -> DbResult<()> {
        messages::truncate_messages(&self.conn, chat_id, after_message_id)
    }

    pub fn get_message(&self, message_id: i64) -> DbResult<Option<Message>> {
        messages::get_message(&self.conn, message_id)
    }

    pub fn update_message(&self, message_id: i64, content: &str) -> DbResult<()> {
        messages::update_message(&self.conn, message_id, content)
    }

    pub fn attach_project(&self, chat_id: i64, project_name: &str) -> DbResult<()> {
        projects::attach_project(&self.conn, chat_id, project_name)
    }

    pub fn detach_project(&self, chat_id: i64, project_name: &str) -> DbResult<()> {
        projects::detach_project(&self.conn, chat_id, project_name)
    }

    pub fn get_chat_projects(&self, chat_id: i64) -> DbResult<Vec<(String, bool)>> {
        projects::get_chat_projects(&self.conn, chat_id)
    }

    pub fn mark_system_prompt_added(&self, chat_id: i64, project_name: &str) -> DbResult<()> {
        projects::mark_system_prompt_added(&self.conn, chat_id, project_name)
    }
}
