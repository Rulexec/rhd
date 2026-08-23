use rusqlite::Connection;

use crate::DbResult;

pub(crate) fn init(conn: &Connection) -> DbResult<()> {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA foreign_keys=ON;",
    )?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chats (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            active_model TEXT
        );

        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chat_id INTEGER NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            model TEXT,
            FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_messages_chat_id ON messages(chat_id);",
    )?;

    // Create chat_projects table if not exists
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chat_projects (
            chat_id INTEGER NOT NULL,
            project_name TEXT NOT NULL,
            system_prompt_added BOOLEAN NOT NULL DEFAULT 0,
            attached_at TEXT NOT NULL,
            PRIMARY KEY (chat_id, project_name),
            FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
        );",
    )?;

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

    // Create messages_queue table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS messages_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chat_id INTEGER NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            model TEXT,
            thinking_content TEXT,
            tool_calls TEXT,
            FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_messages_queue_chat_id ON messages_queue(chat_id);",
    )?;

    // Create message_queue_tags table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS message_queue_tags (
            message_id INTEGER NOT NULL,
            tag TEXT NOT NULL,
            PRIMARY KEY (message_id, tag),
            FOREIGN KEY (message_id) REFERENCES messages_queue(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_message_queue_tags_tag ON message_queue_tags(tag);",
    )?;

    // Create chat_tools table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chat_tools (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            chat_id INTEGER NOT NULL,
            plugin_id TEXT NOT NULL,
            tool_name TEXT NOT NULL,
            tool_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE,
            FOREIGN KEY (plugin_id) REFERENCES plugins(plugin_id) ON DELETE CASCADE,
            UNIQUE(chat_id, tool_name)
        );

        CREATE INDEX IF NOT EXISTS idx_chat_tools_chat_id ON chat_tools(chat_id);
        CREATE INDEX IF NOT EXISTS idx_chat_tools_plugin_id ON chat_tools(plugin_id);",
    )?;

    // Migrate existing tables to add new columns
    migrate(conn)?;

    Ok(())
}

fn migrate(conn: &Connection) -> DbResult<()> {
    // Check if active_model column exists in chats table
    let has_active_model: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='active_model'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;

    if !has_active_model {
        conn.execute_batch("ALTER TABLE chats ADD COLUMN active_model TEXT")?;
    }

    // Check if model column exists in messages table
    let has_model: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name='model'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;

    if !has_model {
        conn.execute_batch("ALTER TABLE messages ADD COLUMN model TEXT")?;
    }

    // Check if thinking_content column exists in messages table
    let has_thinking_content: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name='thinking_content'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;

    if !has_thinking_content {
        conn.execute_batch("ALTER TABLE messages ADD COLUMN thinking_content TEXT")?;
    }

    // Check if tool_calls column exists in messages table
    let has_tool_calls: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name='tool_calls'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;

    if !has_tool_calls {
        conn.execute_batch("ALTER TABLE messages ADD COLUMN tool_calls TEXT")?;
    }

    Ok(())
}
