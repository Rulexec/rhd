---
name: subplan-orchestrate
description: Orchestrates implementation of multiple phase sub-plans from a milestone plan by spawning separate Code mode sessions for each subplan, then validates the complete implementation. Use when given a grand-plan and a list of subplan files to implement sequentially.
disable-model-invocation: true
---

# Subplan Orchestration Skill

## Overview

This skill orchestrates the implementation of multiple phase sub-plans from a milestone plan. It spawns separate Code mode sessions to implement each subplan using the `subplan-implement` skill, then performs final validation of the complete implementation.

## Input Requirements

You will be provided with:
1. **Grand Plan Path**: Path to the milestone plan file (e.g., `plans/rhd-chat-server-implementation.md`)
2. **Subplans Location**: Path to directory or pattern containing phase sub-plan files (e.g., `plans/rhd_chat_server/`)

## Process

### 1. Load and Understand Context

**Read the Grand Plan:**
- Understand the overall architecture and goals
- Identify all phases and their dependencies
- Note the expected final state after all phases complete
- Identify integration points between phases

**Discover Subplan Files:**
- List all files in the subplans location
- Filter for phase sub-plan files (typically `phase-*.md` or similar pattern)
- Sort files in dependency order (phase-1, phase-2, etc.)
- Verify all expected phases are present

**Build Execution Plan:**
- Create ordered list of subplans to implement
- Note any phases that can be parallelized (if dependencies allow)
- Identify critical path and potential blockers

### 2. Implement Subplans Sequentially

For each subplan in dependency order:

**Spawn Code Mode Session:**
- Use `new_task` tool with mode: "code"
- Provide clear message including:
  - Grand plan path (for context)
  - Specific subplan file path to implement
  - Instruction to use `subplan-implement` skill
  - Any relevant context from previous phases

**Example Task Message:**
```
Implement phase sub-plan using subplan-implement skill.

Grand plan: plans/rhd-chat-server-implementation.md
Phase sub-plan: plans/rhd_chat_server/phase-1-database-extensions.md

Context from previous phases: None (this is the first phase)

Follow the subplan-implement skill process exactly:
1. Read and understand both plans
2. Implement the phase
3. Validate with checks and tests
4. Commit changes following conventions
```

**Wait for Completion:**
- The spawned session will implement, validate, and commit
- Review the completion result
- Verify success before proceeding to next phase

**Handle Failures:**
- If a phase fails, stop execution
- Report the failure with details
- Do not proceed to subsequent phases until the issue is resolved

### 3. Final Validation

After all subplans are successfully implemented:

**Spawn Final Code Mode Session:**
- Use `new_task` tool with mode: "code"
- Provide comprehensive validation instructions

**Example Validation Task Message:**
```
Validate complete implementation of grand plan.

Grand plan: plans/rhd-chat-server-implementation.md
All phases implemented:
- Phase 1: plans/rhd_chat_server/phase-1-database-extensions.md
- Phase 2: plans/rhd_chat_server/phase-2-websocket-server-core.md
- Phase 3: plans/rhd_chat_server/phase-3-request-handlers.md
- Phase 4: plans/rhd_chat_server/phase-4-subscription-system.md
- Phase 5: plans/rhd_chat_server/phase-5-plugin-management-system.md
- Phase 6: plans/rhd_chat_server/phase-6-integration-and-testing.md

Perform comprehensive validation:
1. Run all checks: mise run check-cargo
2. Run all tests: mise run test-cargo
3. Verify all requirements from grand plan are met
4. Check integration points between phases
5. Review code quality and consistency
6. Ensure no regressions or broken functionality
7. Report validation results with pass/fail status
```

**Review Validation Results:**
- Confirm all checks pass
- Confirm all tests pass
- Verify grand plan requirements are satisfied
- Identify any remaining issues or gaps

### 4. Report Results

**Success Report:**
- List all implemented phases
- Summarize validation results
- Confirm grand plan completion
- Note any follow-up actions if needed

**Failure Report:**
- Identify which phase or validation step failed
- Provide error details and context
- Suggest remediation steps
- Do not mark task as complete until resolved

## Best Practices

### Orchestration

**Sequential Execution:**
- Implement phases in strict dependency order
- Wait for each phase to complete before starting the next
- Do not parallelize unless explicitly safe (no shared state)

**Context Passing:**
- Provide grand plan to each session for architectural context
- Include relevant context from previous phases when needed
- Keep task messages clear and focused

**Error Handling:**
- Stop on first failure to prevent cascading issues
- Provide detailed error context for debugging
- Do not attempt to fix failures in orchestration session

### Task Messages

**Be Explicit:**
- State the grand plan path clearly
- State the specific subplan path clearly
- Reference the `subplan-implement` skill explicitly
- Provide context about previous phases when relevant

**Be Concise:**
- Focus on what needs to be done
- Avoid redundant information
- Let the skill handle the process details

**Be Complete:**
- Include all necessary paths and references
- Provide enough context to understand the task
- Specify expected outcomes

### Validation

**Comprehensive Checks:**
- Run all relevant check commands
- Run all relevant test commands
- Verify integration between phases
- Check for regressions

**Clear Reporting:**
- Report pass/fail for each check
- Provide details on any failures
- Summarize overall validation status

## Error Handling

### If a Phase Fails

**Stop Execution:**
- Do not proceed to subsequent phases
- Report the failure immediately
- Include error details and context

**Provide Context:**
- Which phase failed
- What step in the phase failed
- Error messages or test failures
- Suggested next steps

### If Validation Fails

**Identify Root Cause:**
- Which check or test failed
- Is it a regression from a recent phase
- Is it an integration issue between phases
- Is it a missing requirement from the grand plan

**Report Clearly:**
- List all failures
- Provide error details
- Suggest remediation approach
- Do not mark task complete until resolved

### If Subplans Are Missing

**Verify Completeness:**
- Check that all expected phases are present
- Verify phase files are readable
- Confirm phase order is correct

**Report Gaps:**
- List missing phases
- Suggest where to find or create them
- Do not proceed with incomplete set

## Checklist

Before starting orchestration:

- [ ] Grand plan path is valid and readable
- [ ] Subplans location is valid and contains expected files
- [ ] All phase sub-plans are present and in correct order
- [ ] Dependencies between phases are understood
- [ ] Validation commands are known and tested

During orchestration:

- [ ] Each phase is implemented in correct order
- [ ] Each phase completes successfully before next starts
- [ ] Context is passed clearly to each session
- [ ] Failures are caught and reported immediately

After orchestration:

- [ ] All phases are implemented
- [ ] Final validation passes all checks
- [ ] Final validation passes all tests
- [ ] Grand plan requirements are satisfied
- [ ] Results are reported clearly

## Example Workflow

1. **Receive inputs:**
   - Grand plan: `plans/rhd-chat-server-implementation.md`
   - Subplans location: `plans/rhd_chat_server/`

2. **Load context:**
   - Read grand plan to understand architecture
   - Discover phase files: phase-1 through phase-6
   - Build execution plan in dependency order

3. **Orchestrate implementation:**
   - Spawn Code session for phase-1 with subplan-implement skill
   - Wait for completion, verify success
   - Spawn Code session for phase-2 with subplan-implement skill
   - Wait for completion, verify success
   - Continue for all phases...

4. **Final validation:**
   - Spawn Code session for comprehensive validation
   - Run all checks and tests
   - Verify grand plan requirements
   - Report results

5. **Report completion:**
   - Summarize all implemented phases
   - Report validation status
   - Confirm grand plan completion

## Conclusion

This skill provides systematic orchestration of multi-phase implementations. By spawning separate Code mode sessions for each phase and performing comprehensive final validation, it ensures that complex, multi-step plans are implemented correctly and completely. The sequential approach with clear error handling prevents cascading failures and makes debugging straightforward.
