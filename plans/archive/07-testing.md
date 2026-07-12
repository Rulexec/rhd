# Phase 7: Testing

## Goal
Comprehensive test coverage for projects feature.

## Current State Analysis
- Backend E2E tests in [`packages/rhd_test/src/`](packages/rhd_test/src/) - uses mock server
- Frontend E2E tests in [`frontend/src/tests/e2e/`](frontend/src/tests/e2e/) - uses Vitest
- Unit tests embedded in modules with `#[cfg(test)]`
- Test utilities in [`packages/rhd_test/src/utils.rs`](packages/rhd_test/src/utils.rs)
- Mock server in [`packages/rhd_test/src/mock_server.rs`](packages/rhd_test/src/mock_server.rs)

## Subtasks

### 7.1. Unit tests for project loader
**File**: `packages/rhd_app/src/project_loader.rs`

**Tests**:
- Load valid project with mcp.yaml and systemPrompt.md
- Load project with only mcp.yaml (no system prompt)
- Load project with only systemPrompt.md (no MCP)
- Handle missing project directory (return empty list)
- Handle invalid YAML (return error with location)
- Handle empty mcp.yaml (skip project)
- Handle multiple projects in directory
- Project name derived from directory name

**Test data**: Create `test_e2e/projects/` with sample projects

### 7.2. Unit tests for ProjectManager
**File**: `packages/rhd_app/src/project_manager.rs`

**Tests**:
- List projects returns all loaded projects
- Get project by name (found/not found)
- Spawn MCP servers (success case)
- Spawn MCP servers (failure case - invalid cmd)
- Track MCP status (connecting → connected)
- Track MCP status (connecting → failed)
- Get MCP clients for project (after spawn)
- Get MCP clients for project (before spawn - empty)
- Multiple projects with multiple MCPs each

**Mock**: Use mock MCP server from [`packages/rhd_test/src/mock_server.rs`](packages/rhd_test/src/mock_server.rs)

### 7.3. Unit tests for Chat DB extensions
**File**: [`packages/rhd_db/src/chat_db.rs`](packages/rhd_db/src/chat_db.rs)

**Tests**:
- Attach project to chat
- Detach project from chat
- Get chat projects (empty)
- Get chat projects (multiple projects)
- Mark system prompt added
- Cascade delete on chat delete (chat_projects removed)
- Attach same project twice (error or idempotent)
- Detach non-attached project (no error)

**Test setup**: Create temporary DB for each test

### 7.4. Unit tests for ChatManager project methods
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs)

**Tests**:
- Attach project spawns MCP servers
- Attach project adds to DB
- Detach project removes from DB
- System prompt injection on first send
- System prompt not duplicated on subsequent sends
- MCP status check before send (all connected - success)
- MCP status check before send (some failed - error)
- Get chat projects returns attached projects

**Mock**: Mock ProjectManager and McpServerCache

### 7.5. Unit tests for tool execution
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs)

**Tests**:
- Tool call loop (single tool call)
- Tool call loop (multiple tool calls)
- Tool call loop (no tool calls - immediate response)
- Cancel during tool execution (abort_chat)
- Pause during tool execution (pause_chat)
- Resume during tool execution (resume_chat)
- User message during pause (append to history)
- Max iterations guard (stop after N iterations)
- Tool error handling (error returned as result)

**Mock**: Mock AI client and MCP clients

### 7.6. E2E tests for project attachment
**File**: `packages/rhd_test/src/project_test.rs` (new)

**Tests**:
- List projects via WebSocket
- Attach project to chat
- Verify MCP servers started (check status)
- Detach project
- Verify project detached from chat
- Attach multiple projects
- System prompt injected on first message

**Test setup**:
- Create test projects in `test_e2e/projects/`
- Start daemon with test projects directory
- Use WebSocket client to send requests

### 7.7. E2E tests for tool execution in chat
**File**: `packages/rhd_test/src/project_test.rs`

**Tests**:
- Send message with tools attached
- Verify tool calls executed
- Verify tool results in messages
- Cancel tool execution (abort)
- Pause/resume tool execution
- User message during pause

**Test setup**:
- Use mock MCP server with test tools
- Attach project with mock MCP
- Send message, verify tool loop

### 7.8. Frontend E2E tests
**File**: `frontend/src/tests/e2e/projects.test.ts` (new)

**Tests**:
- Display projects panel
- Show available projects
- Attach project to chat
- Detach project from chat
- Show MCP status (connecting/connected/failed)
- Display tool calls in messages
- Cancel/pause/resume buttons visible during tool execution
- Send button disabled when MCP not connected

**Test setup**:
- Use existing frontend test infrastructure
- Mock WebSocket responses
- Test UI interactions

## Deliverables
- [ ] Unit tests for project loader
- [ ] Unit tests for ProjectManager
- [ ] Unit tests for Chat DB extensions
- [ ] Unit tests for ChatManager project methods
- [ ] Unit tests for tool execution
- [ ] E2E tests for project attachment
- [ ] E2E tests for tool execution
- [ ] Frontend E2E tests

## Dependencies
- All previous phases must be complete

## Risk Assessment
- **Medium risk**: Tool execution tests require complex mocking
- **Unknown**: How to test pause/resume reliably (timing issues)
- **Mitigation**: 
  - Use mock MCP server for predictable behavior
  - Use async/await patterns for pause/resume tests
  - Increase timeouts for E2E tests

## Test Data

### Sample project structure
```
test_e2e/projects/
├── test-project-a/
│   ├── mcp.yaml
│   └── systemPrompt.md
├── test-project-b/
│   └── mcp.yaml
└── test-project-empty/
    └── (no files)
```

### Sample mcp.yaml
```yaml
- name: test-mcp
  cmd: "node"
  args: ["test-mcp-server.js"]
  env:
    TEST_VAR: "value"
```

### Sample systemPrompt.md
```markdown
You are a helpful assistant with access to test tools.
```
