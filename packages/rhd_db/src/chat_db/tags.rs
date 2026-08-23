use rusqlite::Connection;
use std::sync::Mutex;

use crate::{DbError, DbResult};

/// Get all tags for a chat.
pub fn get_chat_tags(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<Vec<String>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT tag FROM chat_tags WHERE chat_id = ? ORDER BY tag"
    )?;
    let tags = stmt
        .query_map([chat_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tags)
}

/// Get all tags for a message.
pub fn get_message_tags(conn: &Mutex<Connection>, message_id: i64) -> DbResult<Vec<String>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT tag FROM message_tags WHERE message_id = ? ORDER BY tag"
    )?;
    let tags = stmt
        .query_map([message_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tags)
}

/// Set tags for a chat (replaces all existing tags).
pub fn set_chat_tags(conn: &Mutex<Connection>, chat_id: i64, tags: &[String]) -> DbResult<i64> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn_guard.unchecked_transaction()?;
    
    // Delete existing tags
    tx.execute("DELETE FROM chat_tags WHERE chat_id = ?", [chat_id])?;
    
    // Insert new tags
    for tag in tags {
        tx.execute(
            "INSERT OR IGNORE INTO chat_tags (chat_id, tag) VALUES (?, ?)",
            rusqlite::params![chat_id, tag.as_str()],
        )?;
    }
    
    // Increment chat version
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        rusqlite::params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        rusqlite::params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(new_version)
}

/// Set tags for a message (replaces all existing tags).
pub fn set_message_tags(conn: &Mutex<Connection>, message_id: i64, tags: &[String]) -> DbResult<i64> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn_guard.unchecked_transaction()?;
    
    // Get chat_id from message
    let chat_id: i64 = tx.query_row(
        "SELECT chat_id FROM messages WHERE id = ?1",
        rusqlite::params![message_id],
        |row| row.get(0),
    )?;
    
    // Delete existing tags
    tx.execute("DELETE FROM message_tags WHERE message_id = ?", [message_id])?;
    
    // Insert new tags
    for tag in tags {
        tx.execute(
            "INSERT OR IGNORE INTO message_tags (message_id, tag) VALUES (?, ?)",
            rusqlite::params![message_id, tag.as_str()],
        )?;
    }
    
    // Increment chat version
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        rusqlite::params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        rusqlite::params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(new_version)
}

/// Add tags to a chat (appends, no-op if tag exists).
pub fn add_chat_tags(conn: &Mutex<Connection>, chat_id: i64, tags: &[String]) -> DbResult<i64> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn_guard.unchecked_transaction()?;
    
    for tag in tags {
        tx.execute(
            "INSERT OR IGNORE INTO chat_tags (chat_id, tag) VALUES (?, ?)",
            rusqlite::params![chat_id, tag.as_str()],
        )?;
    }
    
    // Increment chat version
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        rusqlite::params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        rusqlite::params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(new_version)
}

/// Add tags to a message (appends, no-op if tag exists).
pub fn add_message_tags(conn: &Mutex<Connection>, message_id: i64, tags: &[String]) -> DbResult<i64> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn_guard.unchecked_transaction()?;
    
    // Get chat_id from message
    let chat_id: i64 = tx.query_row(
        "SELECT chat_id FROM messages WHERE id = ?1",
        rusqlite::params![message_id],
        |row| row.get(0),
    )?;
    
    for tag in tags {
        tx.execute(
            "INSERT OR IGNORE INTO message_tags (message_id, tag) VALUES (?, ?)",
            rusqlite::params![message_id, tag.as_str()],
        )?;
    }
    
    // Increment chat version
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        rusqlite::params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        rusqlite::params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(new_version)
}

/// Remove tags from a chat (no-op if tag doesn't exist).
pub fn remove_chat_tags(conn: &Mutex<Connection>, chat_id: i64, tags: &[String]) -> DbResult<i64> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn_guard.unchecked_transaction()?;
    
    for tag in tags {
        tx.execute(
            "DELETE FROM chat_tags WHERE chat_id = ? AND tag = ?",
            rusqlite::params![chat_id, tag.as_str()],
        )?;
    }
    
    // Increment chat version
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        rusqlite::params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        rusqlite::params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(new_version)
}

/// Remove tags from a message (no-op if tag doesn't exist).
pub fn remove_message_tags(conn: &Mutex<Connection>, message_id: i64, tags: &[String]) -> DbResult<i64> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn_guard.unchecked_transaction()?;
    
    // Get chat_id from message
    let chat_id: i64 = tx.query_row(
        "SELECT chat_id FROM messages WHERE id = ?1",
        rusqlite::params![message_id],
        |row| row.get(0),
    )?;
    
    for tag in tags {
        tx.execute(
            "DELETE FROM message_tags WHERE message_id = ? AND tag = ?",
            rusqlite::params![message_id, tag.as_str()],
        )?;
    }
    
    // Increment chat version
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        rusqlite::params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        rusqlite::params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(new_version)
}

/// Get all tags for a queue message.
pub fn get_queue_message_tags(conn: &Mutex<Connection>, message_id: i64) -> DbResult<Vec<String>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT tag FROM message_queue_tags WHERE message_id = ? ORDER BY tag"
    )?;
    let tags = stmt
        .query_map([message_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tags)
}

/// Set tags for a queue message (replaces all existing tags).
pub fn set_queue_message_tags(conn: &Mutex<Connection>, message_id: i64, tags: &[String]) -> DbResult<i64> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn_guard.unchecked_transaction()?;
    
    // Get chat_id from queue message
    let chat_id: i64 = tx.query_row(
        "SELECT chat_id FROM messages_queue WHERE id = ?1",
        rusqlite::params![message_id],
        |row| row.get(0),
    )?;
    
    // Delete existing tags
    tx.execute("DELETE FROM message_queue_tags WHERE message_id = ?", [message_id])?;
    
    // Insert new tags
    for tag in tags {
        tx.execute(
            "INSERT OR IGNORE INTO message_queue_tags (message_id, tag) VALUES (?, ?)",
            rusqlite::params![message_id, tag.as_str()],
        )?;
    }
    
    // Increment chat version
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        rusqlite::params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        rusqlite::params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(new_version)
}

/// Add tags to a queue message (appends, no-op if tag exists).
pub fn add_queue_message_tags(conn: &Mutex<Connection>, message_id: i64, tags: &[String]) -> DbResult<i64> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn_guard.unchecked_transaction()?;
    
    // Get chat_id from queue message
    let chat_id: i64 = tx.query_row(
        "SELECT chat_id FROM messages_queue WHERE id = ?1",
        rusqlite::params![message_id],
        |row| row.get(0),
    )?;
    
    for tag in tags {
        tx.execute(
            "INSERT OR IGNORE INTO message_queue_tags (message_id, tag) VALUES (?, ?)",
            rusqlite::params![message_id, tag.as_str()],
        )?;
    }
    
    // Increment chat version
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        rusqlite::params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        rusqlite::params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(new_version)
}

/// Remove tags from a queue message (no-op if tag doesn't exist).
pub fn remove_queue_message_tags(conn: &Mutex<Connection>, message_id: i64, tags: &[String]) -> DbResult<i64> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let tx = conn_guard.unchecked_transaction()?;
    
    // Get chat_id from queue message
    let chat_id: i64 = tx.query_row(
        "SELECT chat_id FROM messages_queue WHERE id = ?1",
        rusqlite::params![message_id],
        |row| row.get(0),
    )?;
    
    for tag in tags {
        tx.execute(
            "DELETE FROM message_queue_tags WHERE message_id = ? AND tag = ?",
            rusqlite::params![message_id, tag.as_str()],
        )?;
    }
    
    // Increment chat version
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE chats SET updated_at = ?1, version = version + 1 WHERE id = ?2",
        rusqlite::params![now, chat_id],
    )?;
    let new_version: i64 = tx.query_row(
        "SELECT version FROM chats WHERE id = ?1",
        rusqlite::params![chat_id],
        |row| row.get(0),
    )?;
    tx.commit()?;
    Ok(new_version)
}
