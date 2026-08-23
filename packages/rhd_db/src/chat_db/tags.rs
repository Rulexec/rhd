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
pub fn set_chat_tags(conn: &Mutex<Connection>, chat_id: i64, tags: &[String]) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    
    // Delete existing tags
    conn_guard.execute("DELETE FROM chat_tags WHERE chat_id = ?", [chat_id])?;
    
    // Insert new tags
    for tag in tags {
        conn_guard.execute(
            "INSERT OR IGNORE INTO chat_tags (chat_id, tag) VALUES (?, ?)",
            rusqlite::params![chat_id, tag.as_str()],
        )?;
    }
    
    Ok(())
}

/// Set tags for a message (replaces all existing tags).
pub fn set_message_tags(conn: &Mutex<Connection>, message_id: i64, tags: &[String]) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    
    // Delete existing tags
    conn_guard.execute("DELETE FROM message_tags WHERE message_id = ?", [message_id])?;
    
    // Insert new tags
    for tag in tags {
        conn_guard.execute(
            "INSERT OR IGNORE INTO message_tags (message_id, tag) VALUES (?, ?)",
            rusqlite::params![message_id, tag.as_str()],
        )?;
    }
    
    Ok(())
}

/// Add tags to a chat (appends, no-op if tag exists).
pub fn add_chat_tags(conn: &Mutex<Connection>, chat_id: i64, tags: &[String]) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    
    for tag in tags {
        conn_guard.execute(
            "INSERT OR IGNORE INTO chat_tags (chat_id, tag) VALUES (?, ?)",
            rusqlite::params![chat_id, tag.as_str()],
        )?;
    }
    
    Ok(())
}

/// Add tags to a message (appends, no-op if tag exists).
pub fn add_message_tags(conn: &Mutex<Connection>, message_id: i64, tags: &[String]) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    
    for tag in tags {
        conn_guard.execute(
            "INSERT OR IGNORE INTO message_tags (message_id, tag) VALUES (?, ?)",
            rusqlite::params![message_id, tag.as_str()],
        )?;
    }
    
    Ok(())
}

/// Remove tags from a chat (no-op if tag doesn't exist).
pub fn remove_chat_tags(conn: &Mutex<Connection>, chat_id: i64, tags: &[String]) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    
    for tag in tags {
        conn_guard.execute(
            "DELETE FROM chat_tags WHERE chat_id = ? AND tag = ?",
            rusqlite::params![chat_id, tag.as_str()],
        )?;
    }
    
    Ok(())
}

/// Remove tags from a message (no-op if tag doesn't exist).
pub fn remove_message_tags(conn: &Mutex<Connection>, message_id: i64, tags: &[String]) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    
    for tag in tags {
        conn_guard.execute(
            "DELETE FROM message_tags WHERE message_id = ? AND tag = ?",
            rusqlite::params![message_id, tag.as_str()],
        )?;
    }
    
    Ok(())
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
pub fn set_queue_message_tags(conn: &Mutex<Connection>, message_id: i64, tags: &[String]) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    
    // Delete existing tags
    conn_guard.execute("DELETE FROM message_queue_tags WHERE message_id = ?", [message_id])?;
    
    // Insert new tags
    for tag in tags {
        conn_guard.execute(
            "INSERT OR IGNORE INTO message_queue_tags (message_id, tag) VALUES (?, ?)",
            rusqlite::params![message_id, tag.as_str()],
        )?;
    }
    
    Ok(())
}

/// Add tags to a queue message (appends, no-op if tag exists).
pub fn add_queue_message_tags(conn: &Mutex<Connection>, message_id: i64, tags: &[String]) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    
    for tag in tags {
        conn_guard.execute(
            "INSERT OR IGNORE INTO message_queue_tags (message_id, tag) VALUES (?, ?)",
            rusqlite::params![message_id, tag.as_str()],
        )?;
    }
    
    Ok(())
}

/// Remove tags from a queue message (no-op if tag doesn't exist).
pub fn remove_queue_message_tags(conn: &Mutex<Connection>, message_id: i64, tags: &[String]) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    
    for tag in tags {
        conn_guard.execute(
            "DELETE FROM message_queue_tags WHERE message_id = ? AND tag = ?",
            rusqlite::params![message_id, tag.as_str()],
        )?;
    }
    
    Ok(())
}
