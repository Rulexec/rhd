# Phase 2: Backend - Role Provider & Storage

## Goal

Extend `ProjectProvider` trait to expose role data and add database storage for tracking active role and role injection state per chat.

## Current State Analysis

### ProjectProvider Trait (`packages/rhd_chat/src/lib.rs`)

```rust
#[async_trait]
pub trait ProjectProvider: Send + Sync {
    async fn get_mcp_status(&self, project_name: &str) -> Vec<(String, McpStatus)>;
    fn get_project_system_prompt(&self, project_name: &str) -> Option<String>;
    fn get_project_mcp_refs(&self, project_name: &str) -> Vec<McpRef>;
    async fn get_mcp_clients(&self, project_name: &str) -> Vec<(String, Arc<McpClient>)>;
    async fn spawn_project_mcp(&self, project_name: &str) -> Result<(), String>;
}
```

### ProjectManager (`packages/rhd_app/src/project_manager.rs`)

Implements `ProjectProvider` trait. Holds `HashMap<String, Project>` where `Project` now includes `roles: Vec<Role>` (from Phase 1).

### ChatDb (`packages/rhd_db/src/chat_db.rs`)

- `chats` table: `id`, `title`, `created_at`, `updated_at`, `active_model`
- `chat_projects` table: `chat_id`, `project_name`, `system_prompt_added`, `attached_at`
- Migration pattern: checks for column existence, adds if missing

## Implementation Plan

### 2.1 Extend ProjectProvider Trait

**File: `packages/rhd_chat/src/lib.rs`**

Add role-related methods to the trait:

```rust
use rhd_api::project::{McpRef, Role};

#[async_trait]
pub trait ProjectProvider: Send + Sync {
    // ... existing methods ...
    
    /// Returns all roles defined in a project
    fn get_project_roles(&self, project_name: &str) -> Vec<Role>;
    
    /// Returns the system prompt for a specific role in a project
    fn get_role_system_prompt(&self, project_name: &str, role_name: &str) -> Option<String>;
}
```

### 2.2 Implement Role Methods in ProjectManager

**File: `packages/rhd_app/src/project_manager.rs`**

Add implementations for the new trait methods:

```rust
impl ProjectManager {
    // ... existing methods ...
    
    pub fn get_project_roles(&self, project_name: &str) -> Vec<Role> {
        self.projects
            .get(project_name)
            .map(|p| p.roles.clone())
            .unwrap_or_default()
    }
    
    pub fn get_role_system_prompt(&self, project_name: &str, role_name: &str) -> Option<String> {
        self.projects
            .get(project_name)
            .and_then(|p| p.roles.iter().find(|r| r.name == role_name))
            .map(|r| r.system_prompt.clone())
    }
}

#[async_trait]
impl ProjectProvider for ProjectManager {
    // ... existing trait implementations ...
    
    fn get_project_roles(&self, project_name: &str) -> Vec<Role> {
        ProjectManager::get_project_roles(self, project_name)
    }
    
    fn get_role_system_prompt(&self, project_name: &str, role_name: &str) -> Option<String> {
        ProjectManager::get_role_system_prompt(self, project_name, role_name)
    }
}
```

### 2.3 Add Database Columns for Role Tracking

**File: `packages/rhd_db/src/chat_db.rs`**

Add new columns to `chats` table via migration:

```rust
fn migrate(&self, conn: &Connection) -> DbResult<()> {
    // ... existing migrations ...
    
    // Check if active_role_project column exists
    let has_active_role_project: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='active_role_project'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;
    
    if !has_active_role_project {
        conn.execute_batch("ALTER TABLE chats ADD COLUMN active_role_project TEXT")?;
    }
    
    // Check if active_role_name column exists
    let has_active_role_name: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='active_role_name'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;
    
    if !has_active_role_name {
        conn.execute_batch("ALTER TABLE chats ADD COLUMN active_role_name TEXT")?;
    }
    
    // Check if roles_list_injected column exists
    let has_roles_list_injected: bool = conn
        .prepare("SELECT COUNT(*) FROM pragma_table_info('chats') WHERE name='roles_list_injected'")?
        .query_row([], |row| row.get::<_, i64>(0))?
        > 0;
    
    if !has_roles_list_injected {
        conn.execute_batch("ALTER TABLE chats ADD COLUMN roles_list_injected BOOLEAN NOT NULL DEFAULT 0")?;
    }
    
    Ok(())
}
```

### 2.4 Add Role Tracking Methods to ChatDb

**File: `packages/rhd_db/src/chat_db.rs`**

Add methods for managing role state:

```rust
impl ChatDb {
    // ... existing methods ...
    
    /// Sets the active role for a chat
    pub fn set_active_role(&self, chat_id: i64, project_name: &str, role_name: &str) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let now = now_iso();
        conn.execute(
            "UPDATE chats SET active_role_project = ?1, active_role_name = ?2, updated_at = ?3 WHERE id = ?4",
            params![project_name, role_name, now, chat_id],
        )?;
        Ok(())
    }
    
    /// Clears the active role for a chat (when no role is selected)
    pub fn clear_active_role(&self, chat_id: i64) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let now = now_iso();
        conn.execute(
            "UPDATE chats SET active_role_project = NULL, active_role_name = NULL, updated_at = ?1 WHERE id = ?2",
            params![now, chat_id],
        )?;
        Ok(())
    }
    
    /// Gets the active role for a chat, returns (project_name, role_name) or None
    pub fn get_active_role(&self, chat_id: i64) -> DbResult<Option<(String, String)>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT active_role_project, active_role_name FROM chats WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![chat_id], |row| {
            let project: Option<String> = row.get(0)?;
            let role: Option<String> = row.get(1)?;
            Ok((project, role))
        })?;
        match rows.next() {
            Some(row) => {
                let (project, role) = row?;
                match (project, role) {
                    (Some(p), Some(r)) => Ok(Some((p, r))),
                    _ => Ok(None),
                }
            }
            None => Ok(None),
        }
    }
    
    /// Marks that the roles list prompt has been injected for this chat
    pub fn mark_roles_list_injected(&self, chat_id: i64) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        conn.execute(
            "UPDATE chats SET roles_list_injected = 1 WHERE id = ?1",
            params![chat_id],
        )?;
        Ok(())
    }
    
    /// Checks if the roles list prompt has been injected for this chat
    pub fn has_roles_list_been_injected(&self, chat_id: i64) -> DbResult<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        let injected: bool = conn
            .prepare("SELECT roles_list_injected FROM chats WHERE id = ?1")?
            .query_row(params![chat_id], |row| row.get(0))?;
        Ok(injected)
    }
    
    /// Resets the roles list injection flag (called when new project with roles is attached)
    pub fn reset_roles_list_injected(&self, chat_id: i64) -> DbResult<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| DbError::InitializationError(e.to_string()))?;
        conn.execute(
            "UPDATE chats SET roles_list_injected = 0 WHERE id = ?1",
            params![chat_id],
        )?;
        Ok(())
    }
}
```

### 2.5 Update ChatInfo Struct

**File: `packages/rhd_db/src/chat_db.rs`**

Update `ChatInfo` to include role information:

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatInfo {
    pub id: i64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub active_model: Option<String>,
    pub active_role_project: Option<String>,  // NEW
    pub active_role_name: Option<String>,     // NEW
}
```

Update `list_chats()`, `get_chat()` queries to include new columns:

```rust
pub fn list_chats(&self) -> DbResult<Vec<ChatInfo>> {
    let conn = self.conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT id, title, created_at, updated_at, active_model, active_role_project, active_role_name FROM chats ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(ChatInfo {
            id: row.get(0)?,
            title: row.get(1)?,
            created_at: row.get(2)?,
            updated_at: row.get(3)?,
            active_model: row.get(4)?,
            active_role_project: row.get(5)?,
            active_role_name: row.get(6)?,
        })
    })?;
    // ... rest of method
}
```

### 2.6 Reset Roles List Injection on Project Attach

**File: `packages/rhd_db/src/chat_db.rs`**

When a project is attached, if it has roles, we need to reset the `roles_list_injected` flag so the updated roles list will be re-injected. This logic will be in `projects.rs` (Phase 3), but the DB method is needed here.

Alternatively, we can add a helper method:

```rust
impl ChatDb {
    /// Checks if any attached project has roles that haven't been reflected in the roles list
    /// This is used to determine if we need to re-inject the roles list
    pub fn needs_roles_list_injection<P: ProjectProvider>(
        &self,
        chat_id: i64,
        project_provider: &P,
    ) -> DbResult<bool> {
        let has_been_injected = self.has_roles_list_been_injected(chat_id)?;
        if !has_been_injected {
            return Ok(true);
        }
        
        // Check if any attached project has roles
        let projects = self.get_chat_projects(chat_id)?;
        for (project_name, _) in projects {
            let roles = project_provider.get_project_roles(&project_name);
            if !roles.is_empty() {
                // If roles_list was injected before this project was attached,
                // we need to re-inject. We detect this by checking if the project
                // was attached after the roles_list was injected.
                // For simplicity, we'll use a different approach in Phase 3.
            }
        }
        
        Ok(false)
    }
}
```

Actually, a simpler approach: when attaching a project that has roles, always call `reset_roles_list_injected()`. This ensures the roles list is re-injected with the updated set of roles.

### 2.7 Add Unit Tests

**File: `packages/rhd_db/src/chat_db.rs`**

```rust
#[cfg(test)]
mod tests {
    // ... existing tests ...
    
    #[test]
    fn test_set_and_get_active_role() {
        let path = "test_chat_active_role.db";
        cleanup(path);
        
        let db = ChatDb::new(path).unwrap();
        let chat_id = db.create_chat("Test").unwrap();
        
        // Initially no active role
        assert!(db.get_active_role(chat_id).unwrap().is_none());
        
        // Set active role
        db.set_active_role(chat_id, "project-a", "developer").unwrap();
        let role = db.get_active_role(chat_id).unwrap().unwrap();
        assert_eq!(role.0, "project-a");
        assert_eq!(role.1, "developer");
        
        // Clear active role
        db.clear_active_role(chat_id).unwrap();
        assert!(db.get_active_role(chat_id).unwrap().is_none());
        
        cleanup(path);
    }
    
    #[test]
    fn test_roles_list_injected_flag() {
        let path = "test_chat_roles_injected.db";
        cleanup(path);
        
        let db = ChatDb::new(path).unwrap();
        let chat_id = db.create_chat("Test").unwrap();
        
        // Initially not injected
        assert!(!db.has_roles_list_been_injected(chat_id).unwrap());
        
        // Mark as injected
        db.mark_roles_list_injected(chat_id).unwrap();
        assert!(db.has_roles_list_been_injected(chat_id).unwrap());
        
        // Reset
        db.reset_roles_list_injected(chat_id).unwrap();
        assert!(!db.has_roles_list_been_injected(chat_id).unwrap());
        
        cleanup(path);
    }
    
    #[test]
    fn test_migration_adds_role_columns() {
        let path = "test_chat_role_migration.db";
        cleanup(path);
        
        // Create database with old schema
        {
            let conn = Connection::open(path).unwrap();
            conn.execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA foreign_keys=ON;
                 CREATE TABLE chats (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     title TEXT NOT NULL,
                     created_at TEXT NOT NULL,
                     updated_at TEXT NOT NULL,
                     active_model TEXT
                 );",
            ).unwrap();
            conn.execute(
                "INSERT INTO chats (title, created_at, updated_at) VALUES ('Old Chat', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z')",
                [],
            ).unwrap();
        }
        
        // Open with ChatDb - should trigger migration
        let db = ChatDb::new(path).unwrap();
        
        // Verify new columns exist and work
        db.set_active_role(1, "proj", "role").unwrap();
        let role = db.get_active_role(1).unwrap().unwrap();
        assert_eq!(role.0, "proj");
        assert_eq!(role.1, "role");
        
        cleanup(path);
    }
}
```

**File: `packages/rhd_app/src/project_manager.rs`**

```rust
#[cfg(test)]
mod tests {
    // ... existing tests ...
    
    #[test]
    fn test_get_project_roles() {
        let mut project = make_project("test", 0);
        project.roles = vec![
            Role {
                name: "developer".to_string(),
                system_prompt: "You are a developer.".to_string(),
                when_to_use: "Use for coding tasks.".to_string(),
            },
            Role {
                name: "reviewer".to_string(),
                system_prompt: "You are a reviewer.".to_string(),
                when_to_use: "Use for code review.".to_string(),
            },
        ];
        let manager = make_manager(vec![project]);
        
        let roles = manager.get_project_roles("test");
        assert_eq!(roles.len(), 2);
        assert_eq!(roles[0].name, "developer");
        assert_eq!(roles[1].name, "reviewer");
    }
    
    #[test]
    fn test_get_role_system_prompt() {
        let mut project = make_project("test", 0);
        project.roles = vec![
            Role {
                name: "developer".to_string(),
                system_prompt: "Dev prompt".to_string(),
                when_to_use: "When coding".to_string(),
            },
        ];
        let manager = make_manager(vec![project]);
        
        let prompt = manager.get_role_system_prompt("test", "developer");
        assert_eq!(prompt, Some("Dev prompt".to_string()));
        
        let prompt = manager.get_role_system_prompt("test", "nonexistent");
        assert_eq!(prompt, None);
        
        let prompt = manager.get_role_system_prompt("nonexistent", "developer");
        assert_eq!(prompt, None);
    }
}
```

## Files to Modify

1. **`packages/rhd_chat/src/lib.rs`**
   - Add `Role` import
   - Add `get_project_roles()` and `get_role_system_prompt()` to `ProjectProvider` trait

2. **`packages/rhd_app/src/project_manager.rs`**
   - Implement `get_project_roles()` method
   - Implement `get_role_system_prompt()` method
   - Add trait implementations
   - Add unit tests

3. **`packages/rhd_db/src/chat_db.rs`**
   - Add migration for `active_role_project`, `active_role_name`, `roles_list_injected` columns
   - Update `ChatInfo` struct with new fields
   - Update `list_chats()` and `get_chat()` queries
   - Add `set_active_role()`, `clear_active_role()`, `get_active_role()` methods
   - Add `mark_roles_list_injected()`, `has_roles_list_been_injected()`, `reset_roles_list_injected()` methods
   - Add unit tests

## Dependencies

- Phase 1 must be complete (Role struct defined)
- No new external dependencies

## Success Criteria

1. ✅ `ProjectProvider` trait extended with `get_project_roles()` and `get_role_system_prompt()`
2. ✅ `ProjectManager` implements new trait methods
3. ✅ Database migration adds role tracking columns
4. ✅ `ChatInfo` includes active role information
5. ✅ `ChatDb` provides methods for role state management
6. ✅ All unit tests pass
7. ✅ Existing tests still pass (backward compatibility via migration)

## Design Decisions

1. **Active role stored on `chats` table**: Simple and efficient. A chat has at most one active role at a time.

2. **`roles_list_injected` flag on `chats` table**: Tracks whether the aggregated roles list prompt has been injected. Reset when a new project with roles is attached.

3. **Role identified by (project_name, role_name)**: Roles are namespaced by project to avoid conflicts. The active role stores both the project name and role name.

4. **Migration-based schema updates**: Follows existing pattern in `ChatDb`. Existing databases are automatically upgraded.

## Next Steps

After this phase:
- Phase 3 will implement role injection logic in `projects.rs`
- Phase 3 will implement conflict detection in `attach_project()`
- Phase 3 will integrate role injection into `stream.rs`
