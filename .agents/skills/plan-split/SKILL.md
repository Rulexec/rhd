---
name: plan-split
description: Splits large implementation plans into manageable, phase-specific plans with detailed implementation guidance. Use when a milestone plan needs to be broken down into per-phase sub-plans.
disable-model-invocation: true
---

# Plan Splitting and Enrichment Skill

## Overview

This skill provides a systematic approach to splitting large implementation plans into manageable, phase-specific plans with detailed implementation guidance.

## Process

### 1. Analyze the Source Plan

**Read and Understand:**
- Read the complete milestone plan to understand the overall architecture
- Identify all phases and their dependencies
- Note the dependency graph and execution order
- Understand the key design decisions

**Identify Components:**
- List all files that need to be modified or created
- Identify data models, APIs, events, and UI components
- Map out the flow of data and events through the system

### 2. Investigate Existing Code

**For Each Phase:**
- Read the actual source files that will be modified
- Understand the existing code structure and patterns
- Identify integration points with existing functionality
- Note naming conventions and coding styles used in the codebase

**Key Files to Examine:**
- Backend: tools, handlers, database, events, WebSocket handlers
- Frontend: types, stores, WebSocket handlers, components
- Shared: API types, protocol definitions

### 3. Create Phase-Specific Plans

**For Each Phase, Include:**

#### Overview Section
- Clear statement of what the phase accomplishes
- Scope boundaries (what's in, what's out)
- Dependencies on other phases

#### Files to Modify/Create
- List each file with its path
- For each file, specify:
  - What functions/methods to add
  - What functions/methods to modify
  - What imports are needed
  - Where in the file to make changes (e.g., "after existing function X")

#### Code Snippets
- Provide complete function signatures
- Include implementation logic with comments
- Show how to integrate with existing code
- Include error handling patterns

#### Data Structures
- Define new types, structs, interfaces
- Show serialization/deserialization if needed
- Include validation logic

#### Event/Message Formats
- Define event names and payloads
- Show JSON structure with examples
- Include all possible states/values

#### Tests
- Unit tests for new functions
- Integration tests for new features
- Edge case tests
- Show test setup and teardown patterns

#### Implementation Notes
- Explain non-obvious design decisions
- Document gotchas and pitfalls
- Explain performance considerations
- Note security implications

#### Dependencies
- List which phases must be completed first
- Note which phases can be done in parallel
- Identify shared resources or types

### 4. Enrichment Strategies

**Be Specific:**
- Use actual function names from the codebase
- Reference actual file paths
- Show exact insertion points in existing code
- Use the same naming conventions as the codebase

**Be Complete:**
- Include all necessary imports
- Show complete function implementations
- Include error handling
- Show how to handle edge cases

**Be Consistent:**
- Use the same terminology across all phase plans
- Ensure type names match across phases
- Keep event names consistent
- Maintain consistent code style

**Be Actionable:**
- Write plans that can be implemented without referring back to the milestone plan
- Include enough context to understand why changes are needed
- Provide examples of expected behavior
- Show how to verify the implementation works

### 5. Review and Validation

**Cross-Phase Consistency:**
- Verify type definitions match across phases
- Ensure event names are consistent
- Check that function signatures align
- Validate that dependencies are correctly identified

**Completeness Check:**
- Verify all files from the milestone plan are covered
- Ensure all features are implemented across phases
- Check that tests cover all new functionality
- Confirm error handling is addressed

**Integration Points:**
- Verify phases connect properly
- Ensure data flows correctly between phases
- Check that events are emitted and handled correctly
- Validate WebSocket protocol consistency

## Best Practices

### Plan Structure

**Use Clear Headings:**
```markdown
# Phase N: [Descriptive Name]

## Overview
## Files to Modify
### 1. `path/to/file.rs`
**Additions:**
**Modifications:**
## Tests
## Implementation Notes
## Dependencies
```

**Organize by File:**
- Group all changes for each file together
- Show the complete context for each change
- Include before/after code when modifying existing functions

### Code Examples

**Show Complete Functions:**
- Don't just show the changes, show the complete function
- Include all necessary imports at the top
- Show how the function integrates with existing code

**Use Real Code:**
- Base examples on actual codebase patterns
- Use real type names and function signatures
- Follow the project's coding conventions

### Testing

**Include Multiple Test Types:**
- Unit tests for individual functions
- Integration tests for feature workflows
- Edge case tests for error conditions
- Performance tests if relevant

**Show Test Patterns:**
- Setup and teardown code
- Mock objects and test data
- Assertions and expected outcomes
- Cleanup code

### Documentation

**Explain the "Why":**
- Don't just show what to do, explain why
- Document design decisions
- Note trade-offs and alternatives considered

**Include Examples:**
- Show expected input/output
- Provide usage examples
- Demonstrate edge cases

## Common Pitfalls to Avoid

1. **Incomplete Type Definitions**: Ensure all types are fully defined and match across phases
2. **Missing Error Handling**: Always include error handling in code examples
3. **Inconsistent Naming**: Use the same names for types, functions, and events across all phases
4. **Missing Dependencies**: Clearly document which phases depend on which
5. **Vague Instructions**: Be specific about where and how to make changes
6. **Missing Tests**: Always include tests for new functionality
7. **Ignoring Existing Patterns**: Follow the existing codebase patterns and conventions

## Checklist for Phase Plans

- [ ] Overview clearly states what the phase accomplishes
- [ ] All files to modify/create are listed
- [ ] Code snippets are complete and actionable
- [ ] Type definitions are complete and consistent
- [ ] Event/message formats are defined
- [ ] Tests are included for all new functionality
- [ ] Implementation notes explain non-obvious decisions
- [ ] Dependencies on other phases are documented
- [ ] Integration points with existing code are clear
- [ ] Error handling is addressed
- [ ] Edge cases are considered
- [ ] Code follows project conventions

## Output Format

Create one markdown file per phase with the naming convention:
```
plans/{milestone-prefix}-phase-{N}-{descriptive-name}.md
```

Where `{milestone-prefix}` is a one-two word description of the milestone plan (e.g., `todo-tool`, `user-auth`, `api-refactor`).

Each file should be self-contained and implementable without referring to other phase plans (except for dependencies).

## Example Phase Plan Structure

```markdown
# Phase 1: Backend - Tool Definition & Storage

## Overview
This phase implements [what it does].

## Files to Modify

### 1. `packages/backend/src/tools.rs`

**Add constant:**
```rust
pub const TOOL_NAME: &str = "tool_name";
```

**Add function:**
```rust
pub fn tool_definition() -> ToolDefinition {
    // Complete implementation
}
```

**Modify function:**
```rust
pub fn existing_function() {
    // Show complete function with changes
}
```

### 2. `packages/backend/src/database.rs`

**Add migration:**
```rust
// Migration code
```

**Add methods:**
```rust
pub fn new_method() {
    // Implementation
}
```

## Tests

### Unit Tests
```rust
#[test]
fn test_new_function() {
    // Test implementation
}
```

### Integration Tests
```rust
#[tokio::test]
async fn test_integration() {
    // Test implementation
}
```

## Implementation Notes

1. **Design Decision**: Explain why this approach was chosen
2. **Performance**: Note any performance considerations
3. **Security**: Document security implications

## Dependencies

- This phase depends on Phase X for [reason]
- This phase must be completed before Phase Y
- Phase Z can be done in parallel with this phase
```

## Conclusion

The goal of plan splitting and enrichment is to create actionable, implementable plans that developers can follow without ambiguity. Each phase plan should be comprehensive enough to implement independently (respecting dependencies) and should integrate seamlessly with other phases.
