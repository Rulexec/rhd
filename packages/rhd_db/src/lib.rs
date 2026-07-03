mod chat_db;

use rusqlite::{Connection, params};
use std::sync::Mutex;
use thiserror::Error;

pub use chat_db::{ChatDb, ChatInfo, Message};

#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),
    #[error("Database initialization error: {0}")]
    InitializationError(String),
}

pub type DbResult<T> = Result<T, DbError>;

pub struct ScenarioDb {
    conn: Mutex<Connection>,
}

impl ScenarioDb {
    pub fn new(path: &str) -> DbResult<Self> {
        let conn = Connection::open(path)?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> DbResult<()> {
        let conn = self.conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS meta (
                nextScenarioId INTEGER NOT NULL
            )",
            [],
        )?;

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM meta",
            [],
            |row| row.get(0),
        )?;

        if count == 0 {
            conn.execute(
                "INSERT INTO meta (nextScenarioId) VALUES (1)",
                [],
            )?;
        }

        Ok(())
    }

    pub fn next_id(&self) -> DbResult<u64> {
        let conn = self.conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
        let tx = conn.unchecked_transaction()?;
        
        let current_id: u64 = tx.query_row(
            "SELECT nextScenarioId FROM meta LIMIT 1",
            [],
            |row| row.get(0),
        )?;

        tx.execute(
            "UPDATE meta SET nextScenarioId = ?1",
            params![current_id + 1],
        )?;

        tx.commit()?;

        Ok(current_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_next_id_increments() {
        let db_path = "test_increment.db";
        let _ = fs::remove_file(db_path);
        
        let db = ScenarioDb::new(db_path).unwrap();
        
        let id1 = db.next_id().unwrap();
        let id2 = db.next_id().unwrap();
        let id3 = db.next_id().unwrap();
        
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
        
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_file(format!("{}-wal", db_path));
        let _ = fs::remove_file(format!("{}-shm", db_path));
    }

    #[test]
    fn test_persistence_across_restarts() {
        let db_path = "test_persistence.db";
        let _ = fs::remove_file(db_path);
        
        {
            let db = ScenarioDb::new(db_path).unwrap();
            let id1 = db.next_id().unwrap();
            let id2 = db.next_id().unwrap();
            assert_eq!(id1, 1);
            assert_eq!(id2, 2);
        }
        
        {
            let db = ScenarioDb::new(db_path).unwrap();
            let id3 = db.next_id().unwrap();
            assert_eq!(id3, 3);
        }
        
        let _ = fs::remove_file(db_path);
        let _ = fs::remove_file(format!("{}-wal", db_path));
        let _ = fs::remove_file(format!("{}-shm", db_path));
    }
}
