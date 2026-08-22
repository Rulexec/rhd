# Phase 4: Memory Cleanup

## Overview

This phase updates all memory files to remove references to the legacy packages that were removed in Phase 1. The goal is to ensure documentation accurately reflects the current architecture.

**Scope:**
- Update `memory/MEMORY.md` to remove references to removed packages
- Update `memory/architecture.md` to reflect new package structure
- Update `memory/file-structure.md` to remove deleted directories
- Update feature documentation as needed
- Ensure all memory files are consistent with current state

**Out of scope:**
- Code cleanup in remaining packages
- Adding new features or documentation

## Files to Modify

### 1. `memory/MEMORY.md`

**Sections to update:**

#### Package List Section
Remove references to:
- `rhd_app` (old daemon/client)
- `rhd_fsm` (FSM framework)
- `rhd_test` (test runner)
- `rhd_chat` (chat manager)
- `rhd_ai` (old AI client)
- `rhd_api` (old API types)

Update the package list to show only:
```markdown
## Package Structure

- `rhd_util` - Shared utilities and error types
- `rhd_ai_client` - AI client wrapper (replaces old rhd_ai)
- `rhd_mock_ai_provider` - Mock AI provider for testing
- `rhd_mcp_client` - MCP protocol client
- `rhd_db` - SQLite database layer
- `rhd_chat_api` - Chat API types and protocol definitions
- `rhd_chat_server` - WebSocket server for chat management
- `rhd_chat_client` - WebSocket client for chat server
- `rhd_app` - CLI tool for chat server interaction (NEW)
- `rhd_plugin_ai_completions` - AI completions plugin
```

#### Architecture Section
Update to reflect:
- No more FSM-based architecture
- New CLI tool for manual testing
- Plugin system architecture
- Simplified package dependencies

### 2. `memory/architecture.md`

**Sections to update:**

#### Package Dependencies
Remove all references to:
- `rhd_fsm` dependencies
- `rhd_chat` dependencies
- `rhd_ai` dependencies
- `rhd_api` dependencies

Update dependency graph to show:
```
rhd_app → rhd_chat_client → rhd_chat_api
rhd_plugin_ai_completions → rhd_chat_client, rhd_chat_server
```

#### Component Descriptions
Remove sections about:
- FSM framework and how it works
- Old chat manager architecture
- Old AI client implementation
- Old API types and structures

Add sections about:
- New CLI tool architecture
- How CLI uses chat client
- Plugin system architecture

### 3. `memory/file-structure.md`

**Sections to update:**

Remove directory listings for:
- `packages/rhd_app/` (old structure)
- `packages/rhd_fsm/`
- `packages/rhd_test/`
- `packages/rhd_chat/`
- `packages/rhd_ai/`
- `packages/rhd_api/`

Add directory listing for:
- `packages/rhd_app/` (new CLI structure)
  - `src/main.rs`
  - `src/cli.rs`
  - `src/commands/`
    - `mod.rs`
    - `chats.rs`
    - `messages.rs`
    - `queue.rs`
    - `create_chat.rs`
    - `plugins.rs`

### 4. `memory/features/*.md`

**Files to check and update:**

#### `memory/features/chat.md`
- Remove references to old `rhd_chat` package
- Update to reference `rhd_chat_server` and `rhd_chat_client`
- Add section about CLI tool for chat operations

#### `memory/features/plugins.md`
- Ensure no references to removed packages
- Verify plugin system documentation is accurate

#### `memory/features/testing.md`
- Remove references to `rhd_test` package
- Update testing documentation to reflect current test structure
- Add section about manual testing with CLI

#### Other feature files
- Check all files in `memory/features/` for references to removed packages
- Update or remove sections as needed

## Cleanup Checklist

### Package References
- [ ] No references to `rhd_fsm` in any memory file
- [ ] No references to `rhd_test` in any memory file
- [ ] No references to old `rhd_chat` in any memory file
- [ ] No references to old `rhd_ai` in any memory file
- [ ] No references to old `rhd_api` in any memory file
- [ ] Old `rhd_app` references updated to new CLI tool

### Architecture Documentation
- [ ] Package dependency graph is accurate
- [ ] Component descriptions match current implementation
- [ ] No obsolete architectural patterns described
- [ ] New CLI tool architecture documented

### File Structure
- [ ] All removed package directories removed from listings
- [ ] New CLI tool directory structure added
- [ ] File paths are accurate and up-to-date

### Feature Documentation
- [ ] Chat feature references correct packages
- [ ] Plugin feature documentation is accurate
- [ ] Testing feature updated with CLI manual testing
- [ ] All feature files consistent with current state

## Implementation Notes

1. **Search and replace:** Use grep or similar to find all references to removed packages:
   ```bash
   grep -r "rhd_fsm\|rhd_test\|rhd_chat\|rhd_ai\|rhd_api" memory/
   ```

2. **Context matters:** When removing references, ensure the surrounding text still makes sense. May need to rewrite sentences or paragraphs.

3. **Consistency:** Ensure all memory files use consistent terminology and package names.

4. **Cross-references:** Check that links between memory files still work after updates.

5. **New CLI tool:** When referencing the new `rhd_app`, make it clear this is the CLI tool, not the old daemon/client.

## Dependencies

- Phase 1 must be complete (packages removed)
- Can run in parallel with Phase 2 and Phase 3
- Should be completed before final documentation review

## Success Criteria

- [ ] All memory files updated to remove references to removed packages
- [ ] Architecture documentation reflects current package structure
- [ ] File structure documentation is accurate
- [ ] Feature documentation is consistent and up-to-date
- [ ] No broken links or references in memory files
- [ ] New CLI tool is properly documented
