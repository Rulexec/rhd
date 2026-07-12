# Phase 1: Data Model & Loading

## Goal

Extend the project data model to include roles and load them from disk. This phase establishes the foundation for the roles feature by defining the data structures and implementing the file loading logic.

## Current State Analysis

### Existing Project Structure

**File: `packages/rhd_api/src/project.rs`**
- `Project` struct contains: `name`, `path`, `mcp_configs`, `system_prompt`
- `ProjectInfo` struct contains: `name`, `has_mcp`, `has_system_prompt`
- `McpRef` struct handles MCP server references with optional `id` field

**File: `packages/rhd_app/src/project_loader.rs`**
- `load_projects()` scans `projects/` directory for subdirectories
- `load_project()` loads a single project from a directory
- Loads `mcp.yaml` and `systemPrompt.md` files
- Validates MCP configs and checks for duplicate IDs
- Returns `Project` struct with loaded data

### Role Requirements

Roles are defined in project directories under `roles/<roleName>/`:
- `systemPrompt.md` — the role's system prompt (required)
- `whenToUse.md` — description for when to use this role (required)

## Implementation Plan

### 1.1 Add Role Data Structures

**File: `packages/rhd_api/src/project.rs`**

Add new `Role` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    pub name: String,
    pub system_prompt: String,
    pub when_to_use: String,
}
```

Update `Project` struct to include roles:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub mcp_configs: Vec<McpRef>,
    pub system_prompt: Option<String>,
    pub roles: Vec<Role>,  // NEW FIELD
}
```

Update `ProjectInfo` struct to include role information:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub name: String,
    pub has_mcp: bool,
    pub has_system_prompt: bool,
    pub has_roles: bool,  // NEW FIELD
    pub role_names: Vec<String>,  // NEW FIELD - list of role names for UI
}
```

Update `From<&Project> for ProjectInfo` implementation:

```rust
impl From<&Project> for ProjectInfo {
    fn from(project: &Project) -> Self {
        Self {
            name: project.name.clone(),
            has_mcp: !project.mcp_configs.is_empty(),
            has_system_prompt: project.system_prompt.is_some(),
            has_roles: !project.roles.is_empty(),
            role_names: project.roles.iter().map(|r| r.name.clone()).collect(),
        }
    }
}
```

### 1.2 Implement Role Loading

**File: `packages/rhd_app/src/project_loader.rs`**

Add new function to load roles from a project directory:

```rust
pub fn load_roles(project_dir: &Path) -> Result<Vec<Role>, ProjectLoadError> {
    let roles_dir = project_dir.join("roles");
    
    if !roles_dir.exists() || !roles_dir.is_dir() {
        return Ok(Vec::new());
    }
    
    let mut roles = Vec::new();
    let entries = roles_dir.read_dir().map_err(|source| ProjectLoadError::Io {
        path: roles_dir.display().to_string(),
        source,
    })?;
    
    for entry in entries {
        let entry = entry.map_err(|source| ProjectLoadError::Io {
            path: roles_dir.display().to_string(),
            source,
        })?;
        let role_dir = entry.path();
        
        if !role_dir.is_dir() {
            continue;
        }
        
        let role_name = role_dir
            .file_name()
            .expect("directory must have a name")
            .to_string_lossy()
            .into_owned();
        
        let system_prompt_file = role_dir.join("systemPrompt.md");
        let when_to_use_file = role_dir.join("whenToUse.md");
        
        // Both files are required for a valid role
        if !system_prompt_file.exists() || !when_to_use_file.exists() {
            continue;
        }
        
        let system_prompt_path = system_prompt_file.display().to_string();
        let system_prompt = std::fs::read_to_string(&system_prompt_file)
            .map_err(|source| ProjectLoadError::Io {
                path: system_prompt_path,
                source,
            })?;
        
        let when_to_use_path = when_to_use_file.display().to_string();
        let when_to_use = std::fs::read_to_string(&when_to_use_file)
            .map_err(|source| ProjectLoadError::Io {
                path: when_to_use_path,
                source,
            })?;
        
        roles.push(Role {
            name: role_name,
            system_prompt,
            when_to_use,
        });
    }
    
    // Sort roles by name for consistent ordering
    roles.sort_by(|a, b| a.name.cmp(&b.name));
    
    Ok(roles)
}
```

### 1.3 Integrate Role Loading into Project Loading

**File: `packages/rhd_app/src/project_loader.rs`**

Update `load_project()` function to call `load_roles()`:

```rust
pub fn load_project(project_dir: &Path) -> Result<Project, ProjectLoadError> {
    // ... existing code for loading mcp_configs and system_prompt ...
    
    // Load roles
    let roles = load_roles(project_dir)?;
    
    Ok(Project {
        name: dir_name,
        path: project_dir.to_path_buf(),
        mcp_configs,
        system_prompt,
        roles,  // NEW FIELD
    })
}
```

### 1.4 Add Unit Tests

**File: `packages/rhd_app/src/project_loader.rs`**

Add comprehensive tests for role loading:

```rust
#[cfg(test)]
mod tests {
    // ... existing tests ...
    
    #[test]
    fn test_load_project_with_roles() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("test-project");
        fs::create_dir_all(&project_dir).unwrap();
        
        // Create roles directory
        let roles_dir = project_dir.join("roles");
        fs::create_dir_all(&roles_dir).unwrap();
        
        // Create first role
        let role1_dir = roles_dir.join("developer");
        fs::create_dir_all(&role1_dir).unwrap();
        fs::write(role1_dir.join("systemPrompt.md"), "You are a developer.").unwrap();
        fs::write(role1_dir.join("whenToUse.md"), "Use when coding tasks.").unwrap();
        
        // Create second role
        let role2_dir = roles_dir.join("reviewer");
        fs::create_dir_all(&role2_dir).unwrap();
        fs::write(role2_dir.join("systemPrompt.md"), "You are a code reviewer.").unwrap();
        fs::write(role2_dir.join("whenToUse.md"), "Use when reviewing code.").unwrap();
        
        // Create mcp.yaml (required for project to be valid)
        fs::write(project_dir.join("mcp.yaml"), "mcp: []\n").unwrap();
        
        let project = load_project(&project_dir).unwrap();
        assert_eq!(project.roles.len(), 2);
        assert_eq!(project.roles[0].name, "developer");
        assert_eq!(project.roles[0].system_prompt, "You are a developer.");
        assert_eq!(project.roles[0].when_to_use, "Use when coding tasks.");
        assert_eq!(project.roles[1].name, "reviewer");
    }
    
    #[test]
    fn test_load_project_without_roles() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("no-roles");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join("mcp.yaml"), "mcp: []\n").unwrap();
        
        let project = load_project(&project_dir).unwrap();
        assert!(project.roles.is_empty());
    }
    
    #[test]
    fn test_load_project_with_incomplete_role() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("incomplete-roles");
        fs::create_dir_all(&project_dir).unwrap();
        
        let roles_dir = project_dir.join("roles");
        fs::create_dir_all(&roles_dir).unwrap();
        
        // Create role with only systemPrompt.md (missing whenToUse.md)
        let role_dir = roles_dir.join("incomplete");
        fs::create_dir_all(&role_dir).unwrap();
        fs::write(role_dir.join("systemPrompt.md"), "Incomplete role.").unwrap();
        
        fs::write(project_dir.join("mcp.yaml"), "mcp: []\n").unwrap();
        
        let project = load_project(&project_dir).unwrap();
        // Incomplete role should be skipped
        assert!(project.roles.is_empty());
    }
    
    #[test]
    fn test_load_roles_sorted_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("sorted-roles");
        fs::create_dir_all(&project_dir).unwrap();
        
        let roles_dir = project_dir.join("roles");
        fs::create_dir_all(&roles_dir).unwrap();
        
        // Create roles in non-alphabetical order
        for name in &["zebra", "alpha", "mike"] {
            let role_dir = roles_dir.join(name);
            fs::create_dir_all(&role_dir).unwrap();
            fs::write(role_dir.join("systemPrompt.md"), format!("Role: {}", name)).unwrap();
            fs::write(role_dir.join("whenToUse.md"), format!("When to use: {}", name)).unwrap();
        }
        
        fs::write(project_dir.join("mcp.yaml"), "mcp: []\n").unwrap();
        
        let project = load_project(&project_dir).unwrap();
        assert_eq!(project.roles.len(), 3);
        assert_eq!(project.roles[0].name, "alpha");
        assert_eq!(project.roles[1].name, "mike");
        assert_eq!(project.roles[2].name, "zebra");
    }
    
    #[test]
    fn test_project_info_has_roles() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("with-roles");
        fs::create_dir_all(&project_dir).unwrap();
        
        let roles_dir = project_dir.join("roles");
        fs::create_dir_all(&roles_dir).unwrap();
        
        let role_dir = roles_dir.join("test-role");
        fs::create_dir_all(&role_dir).unwrap();
        fs::write(role_dir.join("systemPrompt.md"), "Test prompt").unwrap();
        fs::write(role_dir.join("whenToUse.md"), "Test when to use").unwrap();
        
        fs::write(project_dir.join("mcp.yaml"), "mcp: []\n").unwrap();
        
        let project = load_project(&project_dir).unwrap();
        let info = ProjectInfo::from(&project);
        assert!(info.has_roles);
        assert_eq!(info.role_names, vec!["test-role"]);
    }
}
```

## Files to Modify

1. **`packages/rhd_api/src/project.rs`**
   - Add `Role` struct
   - Add `roles: Vec<Role>` field to `Project` struct
   - Add `has_roles: bool` and `role_names: Vec<String>` fields to `ProjectInfo` struct
   - Update `From<&Project> for ProjectInfo` implementation

2. **`packages/rhd_app/src/project_loader.rs`**
   - Add `load_roles()` function
   - Update `load_project()` to call `load_roles()`
   - Add comprehensive unit tests

## Dependencies

- No new external dependencies required
- Uses existing `serde` for serialization
- Uses existing `tempfile` for tests

## Success Criteria

1. ✅ `Role` struct defined with `name`, `system_prompt`, `when_to_use` fields
2. ✅ `Project` struct includes `roles: Vec<Role>` field
3. ✅ `ProjectInfo` includes `has_roles` and `role_names` fields
4. ✅ `load_roles()` function scans `roles/` subdirectory
5. ✅ `load_roles()` loads `systemPrompt.md` and `whenToUse.md` for each role
6. ✅ Roles are sorted alphabetically by name
7. ✅ Incomplete roles (missing required files) are skipped
8. ✅ All unit tests pass
9. ✅ Existing project loading tests still pass

## Testing Strategy

1. **Unit Tests**: Test role loading in isolation
   - Test loading project with multiple roles
   - Test loading project without roles directory
   - Test loading project with incomplete roles
   - Test role sorting
   - Test `ProjectInfo` conversion

2. **Integration Tests**: Will be added in later phases when role injection is implemented

## Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| Role directory doesn't exist | Return empty `Vec<Role>`, don't error |
| Role missing required files | Skip incomplete role, continue loading others |
| File read errors | Propagate error with context (file path) |
| Role name conflicts within project | Not checked in this phase (will be checked in Phase 3) |

## Next Steps

After this phase is complete:
- Phase 2 will extend `ProjectProvider` trait to expose role data
- Phase 2 will add database tables to track active role per chat
- Phase 3 will implement role injection logic
