# Phase 1: Project Data Model & Configuration

## Goal
Define project structure, add configuration, implement loader.

## Current State Analysis
- [`DaemonConfig`](packages/rhd_app/src/config.rs:8) has `scenarios_dir`, `models_dir`, `mcp_dir`, `db_dir` fields
- MCP config loading exists in [`mcp_loader.rs`](packages/rhd_app/src/mcp_loader.rs) - can reuse pattern
- Scenario loading pattern in [`scenario/loader.rs`](packages/rhd_app/src/scenario/loader.rs:73) - directory-based loading

## Subtasks

### 1.1. Add `projects_dir` to DaemonConfig
**File**: [`packages/rhd_app/src/config.rs`](packages/rhd_app/src/config.rs)

**Changes**:
- Add `projects_dir: PathBuf` field with `#[serde(default = "default_projects_dir")]`
- Add `default_projects_dir()` function returning `PathBuf::from("projects")`
- Update `Default` impl to include `projects_dir: default_projects_dir()`

**Test**: Config parsing with/without projectsDir

### 1.2. Define Project types
**File**: `packages/rhd_api/src/project.rs` (new)

**Types**:
```rust
use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub mcp_configs: Vec<McpRef>,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRef {
    pub name: String,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}
```

**Note**: Reuse `McpConfig` from `rhd_mcp_client` for actual server spawning. `McpRef` is the reference format from project's `mcp.yaml`.

**Test**: Serialization/deserialization

### 1.3. Implement Project loader
**File**: `packages/rhd_app/src/project_loader.rs` (new)

**Functions**:
```rust
pub fn load_projects(projects_dir: &Path) -> Result<Vec<Project>, ProjectLoadError>
pub fn load_project(project_dir: &Path) -> Result<Project, ProjectLoadError>
```

**Logic**:
- Iterate subdirectories of `projects_dir`
- For each project dir:
  - Parse `mcp.yaml` if exists (same format as scenario MCP refs)
  - Read `systemPrompt.md` if exists
  - Project name = directory name
- Skip directories without `mcp.yaml` or `systemPrompt.md`

**Error handling**:
- Invalid YAML → error with location
- Missing files → skip project or error based on severity
- Empty project dir → skip

**Test**: Load valid project, handle missing files, invalid YAML

### 1.4. Add CLI flag for projects directory
**File**: [`packages/rhd_app/src/cli.rs`](packages/rhd_app/src/cli.rs)

**Changes**:
- Add `--projects-dir` flag to daemon command
- Override config file value when provided

**Test**: CLI parsing

## Deliverables
- [ ] `projectsDir` config field in [`DaemonConfig`](packages/rhd_app/src/config.rs:8)
- [ ] `Project` struct with MCP configs and system prompt in `packages/rhd_api/src/project.rs`
- [ ] `load_projects()` function in `packages/rhd_app/src/project_loader.rs`
- [ ] Unit tests for loader

## Dependencies
- None (first phase)

## Risk Assessment
- **Low risk**: Follows existing patterns from scenario/MCP loading
- **Unknown**: Project directory structure validation requirements
