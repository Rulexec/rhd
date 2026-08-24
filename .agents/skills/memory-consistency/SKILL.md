---
name: memory-consistency
description: Reviews and maintains consistency in the project's memory/knowledge base system. Use for periodic memory maintenance, after significant feature additions, when onboarding new knowledge areas, or when memory files need reorganization.
disable-model-invocation: true
---

# Memory Consistency Check

## Purpose

This skill documents the process for reviewing and maintaining consistency in the project's memory/knowledge base system. Use this skill when:

- Performing periodic memory system maintenance
- After significant feature additions that may have introduced inconsistencies
- When onboarding new knowledge areas
- When memory files have grown and need reorganization

## Documentation Structure Pattern

The memory system follows a two-layer architecture:

### Top-Level Files (Implementation Details)
**Location**: `memory/*.md` (e.g., `chat.md`, `configuration.md`, `scenarios.md`)

**Contains**:
- Crate APIs and function signatures
- Database schemas and table definitions
- Protocol messages and formats
- Internal architecture details
- Key file paths and module locations
- Implementation-specific technical details

**When to read**: When implementing or modifying code in specific areas

### Features Files (Product View)
**Location**: `memory/features/*.md` (e.g., `features/chat.md`, `features/configuration.md`)

**Contains**:
- What the feature does from a user perspective
- User workflows and interactions
- Configuration options and their effects
- Behavior descriptions (what happens, not how)
- Error handling from user perspective
- UI behavior and display logic

**Does NOT contain**:
- Database schemas
- Internal crate APIs
- Protocol details
- Key file paths
- Implementation internals

**When to read**: When understanding what a feature does from a product perspective

## Consistency Check Process

### Step 1: Review All Memory Files

Read through all files in the memory system:
1. Start with `memory/MEMORY.md` (the index)
2. Read each top-level file
3. Read each features file
4. Note any inconsistencies or overlaps

### Step 2: Identify Implementation Details in Features Files

Check each `features/*.md` file for implementation details that should be removed:

**Remove these from features files**:
- Database schemas (CREATE TABLE statements, table definitions)
- WebSocket protocol details (message formats, event structures)
- Internal crate references (ChatManager, OpenAiClient, etc.)
- Key file paths (packages/rhd_*/src/*.rs)
- Protocol message definitions
- Internal function signatures
- Implementation-specific technical details

**Keep in features files**:
- User-facing behavior
- Configuration options
- Workflows and interactions
- Error messages shown to users
- UI behavior descriptions
- Feature capabilities

### Step 3: Relocate Implementation Details to Top-Level Files

When removing implementation details from features files, identify where they should go in top-level files:

**Database schemas** → Move to relevant top-level file:
- Chat database schema → `memory/chat.md` (Chat Database section)
- Project database schema → `memory/chat.md` or create dedicated file if needed
- Scenario database schema → `memory/scenarios.md` or `memory/architecture.md`

**WebSocket protocol details** → Move to `memory/protocols.md`:
- Chat WebSocket events → `protocols.md` (WebSocket Protocol section)
- Scenario WebSocket events → `protocols.md` (WebSocket Protocol section)
- Project WebSocket events → `protocols.md` (WebSocket Protocol section)

**Internal crate APIs** → Move to `memory/architecture.md`:
- ChatManager methods → `architecture.md` (rhd_chat section)
- OpenAiClient methods → `architecture.md` (rhd_ai section)
- Database operations → `architecture.md` (rhd_db section)

**Key file paths** → Move to `memory/file-structure.md`:
- Source file locations → `file-structure.md` (appropriate section)
- Component paths → `file-structure.md` (frontend section)

**Protocol message formats** → Move to `memory/protocols.md`:
- IPC message types → `protocols.md` (IPC Protocol section)
- WebSocket message types → `protocols.md` (WebSocket Protocol section)

**Process for relocation**:
1. Identify the implementation detail being removed from features file
2. Determine which top-level file is most appropriate (see mapping above)
3. Check if the top-level file already contains this information
4. If not, add the information to the appropriate section in the top-level file
5. Ensure no duplication between top-level files

### Step 4: Rewrite Features Files (Product View Only)

For each features file that contains implementation details:

1. **Extract product-view content**: Keep sections about what the feature does, user interactions, configuration
2. **Remove implementation details**: Delete database schemas, protocol details, key files sections
3. **Maintain structure**: Keep logical sections (Purpose, How It Works, Configuration, Error Handling)
4. **Focus on user perspective**: Describe behavior, not implementation

**Example transformation**:

Before (with implementation details):
```markdown
## Data Model

### Chats Table
- `id` — auto-increment primary key
- `title` — chat name
- `active_model` — currently selected model

## WebSocket Protocol

### Requests
- `createChat` — create new chat with title
- `listChats` — get all chats

## Key Files
- Chat database: `packages/rhd_db/src/chat_db.rs`
- Chat manager: `packages/rhd_chat/src/manager.rs`
```

After (product view only):
```markdown
## How It Works

### Chat Lifecycle
1. User creates a new chat with a title
2. User selects a model from available models list
3. User sends messages; AI responds with streaming text
4. Conversations persist across daemon restarts
```

### Step 5: Verify Cross-References

Check that all markdown links in memory files point to valid files:

1. Search for markdown links: `\[[^\]]+\]\([^)]+\.md\)`
2. Verify each referenced file exists
3. Fix any broken links

**Common locations for cross-references**:
- `memory/MEMORY.md` - index with links to all files
- `memory/development.md` - may reference testing files
- Between top-level and features files (should be minimal)

### Step 6: Update MEMORY.md Index

Ensure `memory/MEMORY.md` accurately reflects:
1. All existing memory files
2. Correct "when to read" descriptions
3. Proper categorization (top-level vs features)
4. No references to deleted or moved files

## Checklist

Use this checklist when performing a memory consistency check:

- [ ] Read all memory files (MEMORY.md, top-level files, features files)
- [ ] Identify implementation details in features files
- [ ] For each implementation detail removed, identify where it should go in top-level files
- [ ] Move database schemas to appropriate top-level files (chat.md, scenarios.md, etc.)
- [ ] Move WebSocket protocol details to protocols.md
- [ ] Move internal crate APIs to architecture.md
- [ ] Move key file paths to file-structure.md
- [ ] Remove database schemas from features files
- [ ] Remove WebSocket protocol details from features files
- [ ] Remove "Key Files" sections from features files
- [ ] Remove internal crate references from features files
- [ ] Verify features files contain only product-view content
- [ ] Verify all cross-references are valid
- [ ] Update MEMORY.md index if files were added/removed/moved
- [ ] Test that the pattern is clear and consistent across all files

## Common Issues

### Issue: Features file contains database schema
**Solution**: Remove the entire "Data Model" or "Database" section from features file. Move schema to relevant top-level file (e.g., `chat.md` for chat database, `scenarios.md` for scenario database).

### Issue: Features file contains WebSocket protocol details
**Solution**: Remove "WebSocket Protocol" section with message formats from features file. Move to `protocols.md` under appropriate subsection (Chat Events, Scenario Events, etc.).

### Issue: Features file contains "Key Files" section
**Solution**: Remove the entire "Key Files" section from features file. Move file paths to `file-structure.md` in appropriate section (backend, frontend, etc.).

### Issue: Features file references internal crates
**Solution**: Remove references to `ChatManager`, `OpenAiClient`, etc. from features file. If the API details are important, move to `architecture.md` under the relevant crate section.

### Issue: Overlap between top-level and features files
**Solution**: Ensure clear separation - features = product view, top-level = implementation. Remove duplicates from features files. Check if top-level files need the information and add it there if missing.

### Issue: Implementation detail removed but no home in top-level files
**Solution**: 
1. Check if the information is already in a top-level file (avoid duplication)
2. If not, determine the most appropriate top-level file based on the content type
3. Add the information to that file in a logical section
4. If no appropriate file exists, consider creating a new top-level file

## Benefits

Following this pattern provides:
1. **Clear separation of concerns**: Product behavior vs implementation details
2. **Easier onboarding**: New team members can understand features without implementation complexity
3. **Better maintainability**: Changes to implementation don't require updating product documentation
4. **Reduced duplication**: Each piece of information has one authoritative location
5. **Faster navigation**: Readers know exactly where to find what they need
6. **Complete knowledge**: Implementation details are preserved in top-level files, not lost
