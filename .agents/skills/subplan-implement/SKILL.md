---
name: subplan-implement
description: Implements a single phase sub-plan from a milestone plan and commits changes following project conventions. Use when given a milestone plan and a specific phase sub-plan to implement.
---

# Subplan Implementation Skill

## Overview

This skill implements a single phase sub-plan from a milestone plan, then commits the changes following project conventions.

## Input Requirements

You will be provided with:
1. **Milestone Plan**: The overall plan containing multiple phases (for context)
2. **Phase Sub-Plan**: The specific phase to implement (detailed implementation guide)

## Process

### 1. Understand the Context

**Read the Milestone Plan:**
- Understand the overall architecture and goals
- Identify where this phase fits in the dependency graph
- Note what previous phases should have already been completed

**Read the Phase Sub-Plan:**
- Understand what this specific phase accomplishes
- Review all files to be modified or created
- Note the implementation steps and code snippets
- Identify tests that need to be written

### 2. Implement the Phase

**Follow the Sub-Plan Exactly:**
- Implement changes in the order specified
- Use the exact file paths, function names, and type definitions from the plan
- Include all error handling as specified
- Write all tests as specified

**Implementation Order:**
1. Create/modify data structures and types
2. Implement core logic functions
3. Add integration points (event handlers, API endpoints, etc.)
4. Write unit tests
5. Write integration tests if specified

**Code Quality:**
- Follow existing codebase patterns and conventions
- Use the same naming conventions as the rest of the codebase
- Include all necessary imports
- Ensure code compiles without warnings

### 3. Validate Implementation

**Run Checks:**
- `mise run check-cargo` - Ensure Rust code compiles
- `mise run check-svelte` - Ensure Svelte code compiles (if frontend changes)
- `mise run test-cargo` - Run Rust unit tests
- `mise run test-frontend-unit` - Run frontend unit tests (if applicable)

**Verify Tests Pass:**
- All new tests should pass
- Existing tests should still pass
- No regressions introduced

**Manual Verification:**
- Review the implementation against the sub-plan
- Ensure all requirements from the phase are met
- Check that integration points work correctly

### 4. Commit Changes

**Stage All Changes:**
```bash
git add -A
```

**Create Commit Message:**
- Format: lowercase, no period, concise summary of changes
- Infer from the work completed in this phase
- Examples:
  - "add todo tool definition and storage"
  - "implement websocket protocol for todo updates"
  - "add frontend stores for todo state management"

**Commit:**
```bash
git commit -m "<commit message>"
```

## Best Practices

### Implementation

**Be Precise:**
- Follow the sub-plan exactly as written
- Don't add extra features not specified in the phase
- Don't refactor existing code unless the plan specifies it

**Handle Edge Cases:**
- Implement all error handling specified in the plan
- Handle edge cases mentioned in implementation notes
- Follow the validation logic provided

**Test Coverage:**
- Write all tests specified in the sub-plan
- Include edge case tests
- Ensure tests are comprehensive but focused on this phase's features

### Code Quality

**Follow Conventions:**
- Use existing patterns from the codebase
- Match naming conventions (camelCase in YAML, snake_case in Rust)
- Follow the project's code style

**Keep It Clean:**
- No debug code or console.log statements
- No commented-out code
- No TODO comments unless specified in the plan

### Committing

**Commit Message Guidelines:**
- Short and descriptive (max 50-70 characters)
- Lowercase, no period at the end
- Concise summary of what was implemented
- Focus on the "what" not the "how"

**Examples:**
- ✓ "add todo tool definition and storage"
- ✓ "implement websocket protocol for todo updates"
- ✓ "fix placeholder resolution in system prompts"
- ✗ "Added the todo tool" (past tense, capitalized)
- ✗ "implement websocket protocol." (has period)
- ✗ "WIP" (not descriptive)

**When to Split Commits:**
- Generally, one commit per phase
- If the phase is very large, consider splitting by logical component
- Each commit should be a working, testable state

## Error Handling

### If Implementation Fails

**Compilation Errors:**
- Review the error message carefully
- Check that all imports are correct
- Verify type definitions match the plan
- Ensure function signatures are correct

**Test Failures:**
- Review the test output
- Check that test setup is correct
- Verify expected values match the implementation
- Ensure mocks are properly configured

**Integration Issues:**
- Verify that dependent phases are complete
- Check that integration points are correctly implemented
- Review event/message formats for consistency

### If Prerequisites Not Met

**Missing Dependencies:**
- Implement the missing dependency first (as a separate commit)
- Or implement it as part of this phase if it's small
- Document the additional work in the commit message

**Conflicts with Existing Code:**
- Review the conflict carefully
- Resolve in favor of the sub-plan specifications
- Ensure existing functionality is not broken

## Checklist

Before committing, verify:

- [ ] All files from the sub-plan have been modified/created
- [ ] All functions match the specified signatures
- [ ] All types are defined as specified
- [ ] All error handling is implemented
- [ ] All tests are written and passing
- [ ] Code compiles without warnings
- [ ] No debug code or console.log statements
- [ ] Code follows project conventions
- [ ] Integration points are correctly implemented
- [ ] Commit message follows conventions (lowercase, no period, descriptive)

## Example Workflow

1. **Receive inputs:**
   - Milestone plan: `plans/milestones/todo-tool-plan.md`
   - Phase sub-plan: `plans/todo-phase-1-tool-definition-storage.md`

2. **Read and understand:**
   - Review milestone plan for context
   - Review phase 1 sub-plan for implementation details

3. **Implement:**
   - Add tool definition to `packages/rhd_ai/src/tools.rs`
   - Add storage to `packages/rhd_db/src/todo_db.rs`
   - Write unit tests
   - Write integration tests

4. **Validate:**
   - Run `mise run check-cargo`
   - Run `mise run test-cargo`
   - Verify all tests pass

5. **Commit:**
   - `git add -A`
   - `git commit -m "add todo tool definition and storage"`

## Conclusion

This skill provides a systematic approach to implementing a single phase from a milestone plan. By following the sub-plan exactly, validating thoroughly, and committing with proper conventions, you ensure that each phase is completed correctly and can be integrated seamlessly with other phases.
