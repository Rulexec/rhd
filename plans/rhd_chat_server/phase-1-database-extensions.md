# Phase 1: Database Extensions

## Overview

This phase extends the `rhd_db` package to support tags for chats and messages, plugin registry, and custom events with acknowledgment tracking. These extensions are required by the `rhd_chat_server` package for chat persistence, plugin management, and custom event broadcasting.

**Scope:**
- Add 5 new database tables: `chat_tags`, `message_tags`, `plugins`, `custom_events`, `custom_event_acks`
- Create new modules for tag operations, plugin operations, and custom event operations
- Add public methods to `ChatDb` for all new functionality
- Add unit tests for all new database operations

**Out of scope:**
- WebSocket server implementation (Phase 2)
- Request handlers (Phase 3)
- Subscription system (Phase 4)

## Dependencies

- **None** — This is the first phase and has no dependencies on other phases
- **Prerequisite:** `rhd_chat_api` package must be implemented (already done)

## Files to Modify

### 1. `packages/rhd_db/src/chat_db/schema.rs`

**Add new tables in `init()` function:**

Add the following SQL after the existing `chat_projects` table creation (around line 43):

```rust
// Create chat_tags table
conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS chat_tags (
        chat_id INTEGER NOT NULL,
        tag TEXT NOT NULL,
        PRIMARY KEY (chat_id, tag),
        FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_chat_tags_tag ON chat_tags(tag);",
)?;

// Create message_tags table
conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS message_tags (
        message_id INTEGER NOT NULL,
        tag TEXT NOT NULL,
        PRIMARY KEY (message_id, tag),
        FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_message_tags_tag ON message_tags(tag);",
)?;

// Create plugins table
conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS plugins (
        plugin_id TEXT PRIMARY KEY,
        is_active INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    );",
)?;

// Create custom_events table
conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS custom_events (
        event_id TEXT PRIMARY KEY,
        event_name TEXT NOT NULL,
        sender_plugin_id TEXT,
        additional TEXT,
        created_at TEXT NOT NULL DEFAULT (datetime('now'))
    );",
)?;

// Create custom_event_acks table
conn.execute_batch(
    "CREATE TABLE IF NOT EXISTS custom_event_acks (
        event_id TEXT NOT NULL,
        plugin_id TEXT NOT NULL,
        acked_at TEXT NOT NULL DEFAULT (datetime('now')),
        PRIMARY KEY (event_id, plugin_id),
        FOREIGN KEY (event_id) REFERENCES custom_events(event_id) ON DELETE CASCADE,
        FOREIGN KEY (plugin_id) REFERENCES plugins(plugin_id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_custom_event_acks_plugin ON custom_event_acks(plugin_id);",
)?;
```

**No migration needed** — These are new tables, not column additions. The `CREATE TABLE IF NOT EXISTS` ensures idempotency.

### 2. `packages/rhd_db/src/chat_db/tags.rs` (NEW FILE)

**Create new file with tag operations:**

```rust
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
            [chat_id, tag.as_str()],
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
            [message_id, tag.as_str()],
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
            [chat_id, tag.as_str()],
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
            [message_id, tag.as_str()],
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
            [chat_id, tag.as_str()],
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
            [message_id, tag.as_str()],
        )?;
    }
    
    Ok(())
}

/// Get all chat IDs that have a specific tag.
pub fn get_chats_by_tag(conn: &Mutex<Connection>, tag: &str) -> DbResult<Vec<i64>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT chat_id FROM chat_tags WHERE tag = ? ORDER BY chat_id"
    )?;
    let chat_ids = stmt
        .query_map([tag], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(chat_ids)
}

/// Get all message IDs that have a specific tag.
pub fn get_messages_by_tag(conn: &Mutex<Connection>, tag: &str) -> DbResult<Vec<i64>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT message_id FROM message_tags WHERE tag = ? ORDER BY message_id"
    )?;
    let message_ids = stmt
        .query_map([tag], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(message_ids)
}
```

### 3. `packages/rhd_db/src/chat_db/plugins.rs` (NEW FILE)

**Create new file with plugin operations:**

```rust
use rusqlite::Connection;
use std::sync::Mutex;

use crate::{DbError, DbResult};

/// Plugin information from the database.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub plugin_id: String,
    pub is_active: bool,
    pub created_at: String,
}

/// Register a plugin or mark it as active if it already exists.
pub fn register_plugin(conn: &Mutex<Connection>, plugin_id: &str) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    
    // Check if plugin exists
    let exists: bool = conn_guard
        .prepare("SELECT COUNT(*) FROM plugins WHERE plugin_id = ?")?
        .query_row([plugin_id], |row| row.get::<_, i64>(0))?
        > 0;
    
    if exists {
        // Mark as active
        conn_guard.execute(
            "UPDATE plugins SET is_active = 1 WHERE plugin_id = ?",
            [plugin_id],
        )?;
    } else {
        // Insert new plugin
        conn_guard.execute(
            "INSERT INTO plugins (plugin_id, is_active) VALUES (?, 1)",
            [plugin_id],
        )?;
    }
    
    Ok(())
}

/// Mark a plugin as inactive (called when WebSocket disconnects).
pub fn deactivate_plugin(conn: &Mutex<Connection>, plugin_id: &str) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn_guard.execute(
        "UPDATE plugins SET is_active = 0 WHERE plugin_id = ?",
        [plugin_id],
    )?;
    Ok(())
}

/// Remove a plugin completely.
pub fn remove_plugin(conn: &Mutex<Connection>, plugin_id: &str) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn_guard.execute("DELETE FROM plugins WHERE plugin_id = ?", [plugin_id])?;
    Ok(())
}

/// Get all plugins.
pub fn get_plugins(conn: &Mutex<Connection>) -> DbResult<Vec<PluginInfo>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT plugin_id, is_active, created_at FROM plugins ORDER BY created_at"
    )?;
    let plugins = stmt
        .query_map([], |row| {
            Ok(PluginInfo {
                plugin_id: row.get::<_, String>(0)?,
                is_active: row.get::<_, i64>(1)? != 0,
                created_at: row.get::<_, String>(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(plugins)
}

/// Get a specific plugin.
pub fn get_plugin(conn: &Mutex<Connection>, plugin_id: &str) -> DbResult<Option<PluginInfo>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT plugin_id, is_active, created_at FROM plugins WHERE plugin_id = ?"
    )?;
    let plugin = stmt
        .query_map([plugin_id], |row| {
            Ok(PluginInfo {
                plugin_id: row.get::<_, String>(0)?,
                is_active: row.get::<_, i64>(1)? != 0,
                created_at: row.get::<_, String>(2)?,
            })
        })?
        .next()
        .transpose()?;
    Ok(plugin)
}

/// Check if a plugin exists and is active.
pub fn is_plugin_active(conn: &Mutex<Connection>, plugin_id: &str) -> DbResult<bool> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let is_active: Option<bool> = conn_guard
        .prepare("SELECT is_active FROM plugins WHERE plugin_id = ?")?
        .query_row([plugin_id], |row| Ok(row.get::<_, i64>(0)? != 0))
        .ok();
    Ok(is_active.unwrap_or(false))
}
```

### 4. `packages/rhd_db/src/chat_db/custom_events.rs` (NEW FILE)

**Create new file with custom event operations:**

```rust
use rusqlite::Connection;
use std::sync::Mutex;

use crate::{DbError, DbResult};

/// Custom event information from the database.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomEventInfo {
    pub event_id: String,
    pub event_name: String,
    pub sender_plugin_id: Option<String>,
    pub additional: Option<String>,
    pub created_at: String,
}

/// Create a new custom event.
pub fn create_custom_event(
    conn: &Mutex<Connection>,
    event_id: &str,
    event_name: &str,
    sender_plugin_id: Option<&str>,
    additional: Option<&str>,
) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn_guard.execute(
        "INSERT INTO custom_events (event_id, event_name, sender_plugin_id, additional) VALUES (?, ?, ?, ?)",
        rusqlite::params![event_id, event_name, sender_plugin_id, additional],
    )?;
    Ok(())
}

/// Get a custom event by ID.
pub fn get_custom_event(conn: &Mutex<Connection>, event_id: &str) -> DbResult<Option<CustomEventInfo>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT event_id, event_name, sender_plugin_id, additional, created_at FROM custom_events WHERE event_id = ?"
    )?;
    let event = stmt
        .query_map([event_id], |row| {
            Ok(CustomEventInfo {
                event_id: row.get::<_, String>(0)?,
                event_name: row.get::<_, String>(1)?,
                sender_plugin_id: row.get::<_, Option<String>>(2)?,
                additional: row.get::<_, Option<String>>(3)?,
                created_at: row.get::<_, String>(4)?,
            })
        })?
        .next()
        .transpose()?;
    Ok(event)
}

/// Acknowledge a custom event for a plugin.
pub fn ack_custom_event(conn: &Mutex<Connection>, event_id: &str, plugin_id: &str) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn_guard.execute(
        "INSERT OR IGNORE INTO custom_event_acks (event_id, plugin_id) VALUES (?, ?)",
        [event_id, plugin_id],
    )?;
    Ok(())
}

/// Check if a plugin has acknowledged a custom event.
pub fn has_plugin_acked(conn: &Mutex<Connection>, event_id: &str, plugin_id: &str) -> DbResult<bool> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let count: i64 = conn_guard
        .prepare("SELECT COUNT(*) FROM custom_event_acks WHERE event_id = ? AND plugin_id = ?")?
        .query_row(rusqlite::params![event_id, plugin_id], |row| row.get(0))?;
    Ok(count > 0)
}

/// Get all custom events that a plugin has NOT acknowledged.
pub fn get_pending_events_for_plugin(conn: &Mutex<Connection>, plugin_id: &str) -> DbResult<Vec<CustomEventInfo>> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn_guard.prepare(
        "SELECT ce.event_id, ce.event_name, ce.sender_plugin_id, ce.additional, ce.created_at
         FROM custom_events ce
         WHERE ce.event_id NOT IN (
             SELECT event_id FROM custom_event_acks WHERE plugin_id = ?
         )
         ORDER BY ce.created_at"
    )?;
    let events = stmt
        .query_map([plugin_id], |row| {
            Ok(CustomEventInfo {
                event_id: row.get::<_, String>(0)?,
                event_name: row.get::<_, String>(1)?,
                sender_plugin_id: row.get::<_, Option<String>>(2)?,
                additional: row.get::<_, Option<String>>(3)?,
                created_at: row.get::<_, String>(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(events)
}

/// Delete a custom event (also deletes all acknowledgments due to CASCADE).
pub fn delete_custom_event(conn: &Mutex<Connection>, event_id: &str) -> DbResult<()> {
    let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    conn_guard.execute("DELETE FROM custom_events WHERE event_id = ?", [event_id])?;
    Ok(())
}
```

### 5. `packages/rhd_db/src/chat_db/mod.rs`

**Add module declarations at the top (after line 3):**

```rust
mod tags;
mod plugins;
mod custom_events;
```

**Add public methods to `ChatDb` impl (after line 204):**

```rust
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

    pub fn get_chats_by_tag(&self, tag: &str) -> DbResult<Vec<i64>> {
        tags::get_chats_by_tag(&self.conn, tag)
    }

    pub fn get_messages_by_tag(&self, tag: &str) -> DbResult<Vec<i64>> {
        tags::get_messages_by_tag(&self.conn, tag)
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

    pub fn get_plugins(&self) -> DbResult<Vec<plugins::PluginInfo>> {
        plugins::get_plugins(&self.conn)
    }

    pub fn get_plugin(&self, plugin_id: &str) -> DbResult<Option<plugins::PluginInfo>> {
        plugins::get_plugin(&self.conn, plugin_id)
    }

    pub fn is_plugin_active(&self, plugin_id: &str) -> DbResult<bool> {
        plugins::is_plugin_active(&self.conn, plugin_id)
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

    pub fn get_custom_event(&self, event_id: &str) -> DbResult<Option<custom_events::CustomEventInfo>> {
        custom_events::get_custom_event(&self.conn, event_id)
    }

    pub fn ack_custom_event(&self, event_id: &str, plugin_id: &str) -> DbResult<()> {
        custom_events::ack_custom_event(&self.conn, event_id, plugin_id)
    }

    pub fn has_plugin_acked(&self, event_id: &str, plugin_id: &str) -> DbResult<bool> {
        custom_events::has_plugin_acked(&self.conn, event_id, plugin_id)
    }

    pub fn get_pending_events_for_plugin(&self, plugin_id: &str) -> DbResult<Vec<custom_events::CustomEventInfo>> {
        custom_events::get_pending_events_for_plugin(&self.conn, plugin_id)
    }

    pub fn delete_custom_event(&self, event_id: &str) -> DbResult<()> {
        custom_events::delete_custom_event(&self.conn, event_id)
    }
```

**Re-export PluginInfo and CustomEventInfo at the module level (after line 52):**

```rust
pub use plugins::PluginInfo;
pub use custom_events::CustomEventInfo;
```

### 6. `packages/rhd_db/src/chat_db/tests.rs`

**Add unit tests for new functionality:**

```rust
#[test]
fn test_chat_tags() {
    let db = ChatDb::new(":memory:").unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();
    
    // Initially no tags
    let tags = db.get_chat_tags(chat_id).unwrap();
    assert!(tags.is_empty());
    
    // Set tags
    db.set_chat_tags(chat_id, &["tag1".to_string(), "tag2".to_string()]).unwrap();
    let tags = db.get_chat_tags(chat_id).unwrap();
    assert_eq!(tags, vec!["tag1", "tag2"]);
    
    // Add tags
    db.add_chat_tags(chat_id, &["tag3".to_string()]).unwrap();
    let tags = db.get_chat_tags(chat_id).unwrap();
    assert_eq!(tags, vec!["tag1", "tag2", "tag3"]);
    
    // Add duplicate tag (no-op)
    db.add_chat_tags(chat_id, &["tag1".to_string()]).unwrap();
    let tags = db.get_chat_tags(chat_id).unwrap();
    assert_eq!(tags, vec!["tag1", "tag2", "tag3"]);
    
    // Remove tags
    db.remove_chat_tags(chat_id, &["tag2".to_string()]).unwrap();
    let tags = db.get_chat_tags(chat_id).unwrap();
    assert_eq!(tags, vec!["tag1", "tag3"]);
    
    // Get chats by tag
    let chat_ids = db.get_chats_by_tag("tag1").unwrap();
    assert_eq!(chat_ids, vec![chat_id]);
}

#[test]
fn test_message_tags() {
    let db = ChatDb::new(":memory:").unwrap();
    let chat_id = db.create_chat("Test Chat").unwrap();
    let message_id = db.add_message(chat_id, "user", "Hello", None, None).unwrap();
    
    // Initially no tags
    let tags = db.get_message_tags(message_id).unwrap();
    assert!(tags.is_empty());
    
    // Set tags
    db.set_message_tags(message_id, &["important".to_string()]).unwrap();
    let tags = db.get_message_tags(message_id).unwrap();
    assert_eq!(tags, vec!["important"]);
    
    // Add tags
    db.add_message_tags(message_id, &["urgent".to_string()]).unwrap();
    let tags = db.get_message_tags(message_id).unwrap();
    assert_eq!(tags, vec!["important", "urgent"]);
    
    // Remove tags
    db.remove_message_tags(message_id, &["important".to_string()]).unwrap();
    let tags = db.get_message_tags(message_id).unwrap();
    assert_eq!(tags, vec!["urgent"]);
}

#[test]
fn test_plugins() {
    let db = ChatDb::new(":memory:").unwrap();
    
    // Initially no plugins
    let plugins = db.get_plugins().unwrap();
    assert!(plugins.is_empty());
    
    // Register plugin
    db.register_plugin("plugin-1").unwrap();
    let plugins = db.get_plugins().unwrap();
    assert_eq!(plugins.len(), 1);
    assert_eq!(plugins[0].plugin_id, "plugin-1");
    assert!(plugins[0].is_active);
    
    // Check if active
    assert!(db.is_plugin_active("plugin-1").unwrap());
    
    // Deactivate plugin
    db.deactivate_plugin("plugin-1").unwrap();
    assert!(!db.is_plugin_active("plugin-1").unwrap());
    
    // Re-register (marks as active)
    db.register_plugin("plugin-1").unwrap();
    assert!(db.is_plugin_active("plugin-1").unwrap());
    
    // Remove plugin
    db.remove_plugin("plugin-1").unwrap();
    let plugins = db.get_plugins().unwrap();
    assert!(plugins.is_empty());
}

#[test]
fn test_custom_events() {
    let db = ChatDb::new(":memory:").unwrap();
    
    // Register a plugin first
    db.register_plugin("sender-plugin").unwrap();
    db.register_plugin("receiver-plugin").unwrap();
    
    // Create custom event
    db.create_custom_event(
        "event-1",
        "my-event",
        Some("sender-plugin"),
        Some("{\"key\": \"value\"}"),
    ).unwrap();
    
    // Get event
    let event = db.get_custom_event("event-1").unwrap().unwrap();
    assert_eq!(event.event_id, "event-1");
    assert_eq!(event.event_name, "my-event");
    assert_eq!(event.sender_plugin_id, Some("sender-plugin".to_string()));
    
    // Check pending events for receiver
    let pending = db.get_pending_events_for_plugin("receiver-plugin").unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].event_id, "event-1");
    
    // Acknowledge event
    db.ack_custom_event("event-1", "receiver-plugin").unwrap();
    
    // Check pending again (should be empty)
    let pending = db.get_pending_events_for_plugin("receiver-plugin").unwrap();
    assert!(pending.is_empty());
    
    // Check if acknowledged
    assert!(db.has_plugin_acked("event-1", "receiver-plugin").unwrap());
    
    // Delete event
    db.delete_custom_event("event-1").unwrap();
    let event = db.get_custom_event("event-1").unwrap();
    assert!(event.is_none());
}
```

## Tests

### Unit Tests

All unit tests are in `packages/rhd_db/src/chat_db/tests.rs`:

1. **`test_chat_tags`** — Tests chat tag operations (set, add, remove, get by tag)
2. **`test_message_tags`** — Tests message tag operations (set, add, remove)
3. **`test_plugins`** — Tests plugin registration, activation, deactivation, removal
4. **`test_custom_events`** — Tests custom event creation, acknowledgment, pending events

### Running Tests

```bash
cd packages/rhd_db
cargo test
```

## Implementation Notes

1. **Tag Storage Strategy**: Tags use separate tables with composite primary keys (`chat_id, tag` and `message_id, tag`). This allows efficient querying by tag and simplifies tag addition/removal.

2. **Cascade Delete**: Both `chat_tags` and `message_tags` have `ON DELETE CASCADE` foreign keys, so tags are automatically removed when the parent chat/message is deleted.

3. **Plugin Lifecycle**: Plugins are registered with `is_active = 1`. When a WebSocket disconnects, the server should call `deactivate_plugin()` to mark the plugin as inactive. The plugin can be re-activated by calling `register_plugin()` again.

4. **Custom Event Acknowledgment**: The `custom_event_acks` table tracks which plugins have acknowledged which events. The `get_pending_events_for_plugin()` method returns all events that a plugin has NOT yet acknowledged.

5. **Idempotency**: All operations are idempotent:
   - `INSERT OR IGNORE` for tags and acknowledgments
   - `CREATE TABLE IF NOT EXISTS` for schema
   - `register_plugin()` checks existence before insert/update

6. **Thread Safety**: All database operations use `Mutex<Connection>` to ensure thread safety. The lock is acquired and released within each operation.

## Success Criteria

1. All 5 new tables are created successfully
2. All tag operations work correctly (set, add, remove, get)
3. All plugin operations work correctly (register, deactivate, remove, get)
4. All custom event operations work correctly (create, ack, get pending)
5. All unit tests pass
6. No breaking changes to existing functionality

## Next Steps

After this phase is complete, proceed to **Phase 2: WebSocket Server Core** to create the `rhd_chat_server` package.
