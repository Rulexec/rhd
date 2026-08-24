---
name: plans-archive
description: Archives completed implementation plans and consolidates their knowledge into product feature documentation. Use when plans in plans/ are implemented and no longer active.
disable-model-invocation: true
---

# Plans Archive

## Purpose

This skill documents the process for archiving completed implementation plans and updating the product feature documentation. Use this skill when:

- Plans in `plans/` directory have been implemented and are no longer active
- Need to consolidate plan knowledge into product feature documentation
- Cleaning up the plans directory to keep only active/planned work
- Onboarding new team members who need to understand what's been built

## Process Overview

The plans archive process extracts product-level knowledge from implementation plans and consolidates it into the `memory/features/` documentation, then moves the processed plans to `plans/archive/`.

## Step-by-Step Process

### Step 1: Read All Active Plans

Read all plan files in `plans/` directory (excluding `archive/` and `milestones/` subdirectories).

**Note on subfolders**: Subfolders within `plans/` (other than `archive/`) may contain:
- **Subplans**: Phase-specific implementation plans derived from a grand plan
- **Development states**: Intermediate snapshots or progress tracking files
- **Assets**: Supporting documents, diagrams, or reference materials

These files should be read and analyzed alongside their parent plans to extract complete product-level knowledge. However, after analysis, they should be **removed** (not moved to `plans/archive/`), as they are transient working artifacts rather than standalone completed plans.

For each plan file, extract:
- What feature/capability it implements
- User-facing behavior and workflows
- Configuration options
- Error handling from user perspective
- UI behavior and interactions

**Ignore implementation details**:
- Database schemas
- Internal crate APIs
- Protocol message formats
- Key file paths
- Code snippets and function signatures

### Step 2: Read Existing Feature Documentation

Read all files in `memory/features/` to understand current state.

Identify:
- Which features are already documented
- What gaps exist between plans and documentation
- Which feature files need updates

### Step 3: Update Feature Documentation

For each plan, determine which `memory/features/` file it relates to and update that file with product-level information.

**Add to feature files**:
- New capabilities or workflows
- Configuration options and their effects
- User-facing behavior changes
- Error handling improvements
- UI enhancements

**Example updates**:

If plan adds "delete all chats" feature:
```markdown
## Delete All Chats
- "Delete all chats" button at the bottom of the chat list sidebar
- Removes all chats and their messages at once
- Shows confirmation dialog before deleting
- Disabled when no chats exist
```

If plan adds "smart auto-scrolling":
```markdown
### Smart Auto-Scrolling
- Message list auto-scrolls to bottom when new content arrives **only if user is at bottom**
- Tracks "at bottom" state with 30px threshold from bottom
- If user scrolls up, auto-scroll stops (respects user's scroll position)
- If user scrolls back to bottom, auto-scroll resumes
```

If plan adds new feature not yet documented:
- Create new file in `memory/features/` (e.g., `projects.md`)
- Document the feature from product perspective
- Update `memory/MEMORY.md` index to include the new file

### Step 4: Update MEMORY.md Index

If you created new feature files or significantly updated existing ones:

1. Open `memory/MEMORY.md`
2. Update the "Product Features" table:
   - Add entries for new files
   - Update "when to read" descriptions for modified files
   - Ensure all feature files are listed

**Example**:
```markdown
| [features/projects.md](features/projects.md) | Understanding projects feature, project structure, attaching projects to chats, MCP server lifecycle, system prompt injection |
```

### Step 5: Archive Plans and Clean Up Subfolders

**For top-level plan files in `plans/`**: Move all processed plan files from `plans/` to `plans/archive/`.

**Preferred method**: Use file moving tools provided by your AI assistant (e.g., filesystem MCP tools). Multiple files can be moved in parallel by calling the move tool multiple times in a single message.

**Fallback method**: If no file moving tools are available, use the `mv` command via terminal:
```bash
mv plans/filename.md plans/archive/
```

**For subfolder contents (subplans, development states, assets)**: After analyzing and extracting relevant knowledge, **delete** these files rather than moving them to `plans/archive/`. They are working artifacts tied to the parent plan's development process, not standalone completed plans.

```bash
# Remove subplans, state files, and assets after analysis
rm plans/feature-name/subplan-phase-1.md
rm plans/feature-name/development-state.md
# Remove empty subfolder if no longer needed
rmdir plans/feature-name/
```

**Verify**:
- All processed top-level plans are now in `plans/archive/`
- Subfolders containing subplans/assets have been cleaned up (files removed)
- `plans/` directory only contains active/planned work (if any)
- No plans were accidentally deleted

## What to Extract from Plans

### Extract (Product View)
- User workflows and interactions
- Feature capabilities and behavior
- Configuration options and their effects
- Error messages shown to users
- UI behavior and display logic
- Integration points between features
- New commands or CLI options
- New WebSocket events (user-facing)

### Don't Extract (Implementation Details)
- Database schemas and table definitions
- Internal crate APIs and function signatures
- Protocol message formats (JSON structures)
- Key file paths and module locations
- Code snippets and implementation logic
- Migration steps and technical debt
- Testing implementation details
- Risk assessments and mitigation strategies

## Checklist

Use this checklist when archiving plans:

- [ ] Read all plan files in `plans/` (excluding archive/ and milestones/)
- [ ] Read subfolder contents (subplans, development states, assets) for additional context
- [ ] Read all existing `memory/features/` files
- [ ] For each plan, identify which feature file it relates to
- [ ] Extract product-level information from each plan (including insights from subplans/assets)
- [ ] Update relevant `memory/features/` files with new information
- [ ] Create new feature files if needed (e.g., for entirely new features)
- [ ] Update `memory/MEMORY.md` index if files were added or significantly changed
- [ ] Move all processed top-level plans to `plans/archive/` using file moving tools or `mv` command
- [ ] Remove subfolder contents (subplans, states, assets) after analysis — do not archive them
- [ ] Verify `plans/` directory only contains active work

## Common Issues

### Issue: Plan describes both product and implementation
**Solution**: Extract only the product-view portions. Ignore implementation details like database schemas, internal APIs, and code snippets. Focus on what the user sees and interacts with.

### Issue: Multiple plans relate to the same feature
**Solution**: Consolidate information from all related plans into a single feature file. Ensure no duplication. Later plans may update or supersede earlier plans — use the most recent information.

### Issue: Plan describes a bug fix
**Solution**: If the bug fix introduced new user-facing behavior (e.g., error UI, validation messages), document that behavior. If it just fixed internal logic without user impact, no documentation update needed — just archive the plan.

### Issue: Plan describes internal refactoring
**Solution**: Internal refactoring without user-facing changes doesn't need feature documentation. Archive the plan without updating feature files. If the refactoring changed architecture significantly, consider updating `memory/architecture.md` instead.

### Issue: Feature file already contains the information
**Solution**: Skip updating that section. Don't duplicate information. Only add new or changed information.

## Benefits

Following this process provides:
1. **Up-to-date documentation**: Feature docs reflect what's actually built
2. **Clean plans directory**: Only active work remains in `plans/`
3. **Historical record**: Completed plans preserved in `plans/archive/`
4. **Product focus**: Documentation emphasizes user value, not implementation
5. **Easier onboarding**: New team members can understand features without reading old plans
6. **Reduced confusion**: Single source of truth for feature behavior

## Related Skills

- **Memory Consistency** (`.agents/skills/memory-consistency/SKILL.md`): Use this skill to ensure feature files contain only product-view content and implementation details are in top-level memory files.
