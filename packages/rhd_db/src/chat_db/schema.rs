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

    // Check if active_role_project column exists in chats table
    let has_active_role_project: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='active_role_project'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;

    if !has_active_role_project {
        conn.execute_batch("ALTER TABLE chats ADD COLUMN active_role_project TEXT")?;
    }

    // Check if active_role_name column exists in chats table
    let has_active_role_name: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='active_role_name'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;

    if !has_active_role_name {
        conn.execute_batch("ALTER TABLE chats ADD COLUMN active_role_name TEXT")?;
    }

    // Check if roles_list_injected column exists in chats table
    let has_roles_list_injected: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='roles_list_injected'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;

    if !has_roles_list_injected {
        conn.execute_batch("ALTER TABLE chats ADD COLUMN roles_list_injected BOOLEAN NOT NULL DEFAULT 0")?;
    }

    // Check if role_prompt_pending column exists in chats table
    let has_role_prompt_pending: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='role_prompt_pending'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;

    if !has_role_prompt_pending {
        conn.execute_batch("ALTER TABLE chats ADD COLUMN role_prompt_pending BOOLEAN NOT NULL DEFAULT 0")?;
    }

    // Check if todo_list column exists in chats table
    let has_todo_list: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='todo_list'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;

    if !has_todo_list {
        conn.execute_batch("ALTER TABLE chats ADD COLUMN todo_list TEXT")?;
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
