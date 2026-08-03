# Phase 5: Test Case Creation

## Overview

This phase creates test case markdown files for all pause/abort scenarios. Each test case file documents the detailed steps, expected results, and coverage references following the established pattern from the tests-grooming skill.

**Scope:**
- Create `tests/cases/pause-abort/` directory
- Create 5 test case files matching the grand plan scenarios
- Each file includes detailed steps, expected results, and "Covered By" section
- Deprecate or update old `tests/cases/chat-pause-resume.md`

**Out of scope:**
- E2E test implementation (Phase 6)
- Backend or frontend code changes

**Dependencies:**
- Phase 4 (Frontend Integration) must be completed first (to know final file paths and test names)

## Files to Create

### 1. `tests/cases/pause-abort/pause-during-ai-call.md`

```markdown
# Test Case: Pause During AI Call

## Description
User pauses chat while AI is in thinking or streaming state.

## Preconditions
- Chat exists and is selected
- AI chat call in progress (isStreaming = true, isPaused = false)
- AI is in thinking or final message streaming state

## Steps

### Pause Phase
1. User clicks Pause button
2. System dispatches `pauseChat` action
3. System sends pause request to daemon
4. Daemon waits for AI call to complete naturally
5. If AI emits tool calls, daemon stores them in pending_tool_calls
6. Daemon transitions to Paused state
7. System receives `chatPaused` action
8. System sets isPaused to true
9. System sets isStreaming to false

### Message Queue Phase
10. User types message in input field
11. User clicks Send button
12. System dispatches `queueMessage` action
13. System sends queue request to daemon
14. Daemon stores message in message queue
15. System receives `messageQueued` action
16. System adds message to queuedMessages store with "Queued" indicator

### Resume Phase
17. User clicks Resume button
18. System dispatches `resumeChat` action
19. System sends resume request to daemon
20. Daemon processes pending tool calls (if any)
21. Daemon executes tools, sends results to AI
22. Daemon processes queued messages
23. Daemon appends queued messages to chat history
24. Daemon sends AI chat request with full context
25. Daemon transitions to Running state
26. System receives `chatResumed` action
27. System sets isPaused to false
28. System sets isStreaming to true
29. System clears queuedMessages store

## Expected Results
- Pause: AI completes naturally, tool calls remembered but not executed
- Message queue: Messages stored with "Queued" indicator
- Resume: Pending tool calls executed, queued messages sent, tool loop continues

## Covered By

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) - `pauses during AI call and resumes with pending tool calls` (steps 2-29)

### UI Tests
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders pause button when streaming` (step 1)
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when paused` (step 17)
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `allows message input when paused` (step 10)
```

### 2. `tests/cases/pause-abort/pause-during-tool-execution.md`

```markdown
# Test Case: Pause During Tool Execution

## Description
User pauses chat while tool calls are executing.

## Preconditions
- Chat exists and is selected
- AI chat call completed with tool calls
- Tool calls are executing (isStreaming = true, isPaused = false)

## Steps

### Pause Phase
1. User clicks Pause button
2. System dispatches `pauseChat` action
3. System sends pause request to daemon
4. Daemon waits for current tool calls to complete
5. Daemon inserts tool results into chat messages
6. Daemon does NOT continue to next iteration
7. Daemon transitions to Paused state
8. System receives `chatPaused` action
9. System sets isPaused to true
10. System sets isStreaming to false

### Message Queue Phase
11. User types message in input field
12. User clicks Send button
13. System dispatches `queueMessage` action
14. System sends queue request to daemon
15. Daemon stores message in message queue
16. System receives `messageQueued` action
17. System adds message to queuedMessages store with "Queued" indicator

### Resume Phase
18. User clicks Resume button
19. System dispatches `resumeChat` action
20. System sends resume request to daemon
21. Daemon processes queued messages
22. Daemon appends queued messages to chat history
23. Daemon sends AI chat request with full context
24. Daemon transitions to Running state
25. System receives `chatResumed` action
26. System sets isPaused to false
27. System sets isStreaming to true
28. System clears queuedMessages store

## Expected Results
- Pause: Current tool calls complete, results inserted, tool loop paused
- Message queue: Messages stored with "Queued" indicator
- Resume: Queued messages sent, tool loop continues

## Covered By

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) - `pauses during tool execution and resumes` (steps 2-28)

### UI Tests
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders pause button when streaming` (step 1)
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when paused` (step 18)
```

### 3. `tests/cases/pause-abort/abort-during-ai-call.md`

```markdown
# Test Case: Abort During AI Call

## Description
User aborts chat while AI is in thinking or streaming state.

## Preconditions
- Chat exists and is selected
- AI chat call in progress (isStreaming = true, isPaused = false)
- AI is in thinking or final message streaming state

## Steps

### Abort Phase
1. User clicks Abort button
2. System dispatches `abortChat` action
3. System sends abort request to daemon
4. Daemon cancels current AI request immediately
5. Daemon discards partial response
6. If AI emitted tool calls, daemon does NOT execute them
7. Daemon transitions to Aborted state
8. Daemon emits `streamAborted` event
9. System receives `streamAborted` action
10. System removes streaming message from UI
11. System sets isPaused to true
12. System sets isStreaming to false

### Message Queue Phase
13. User types message in input field
14. User clicks Send button
15. System dispatches `queueMessage` action
16. System sends queue request to daemon
17. Daemon stores message in message queue
18. System receives `messageQueued` action
19. System adds message to queuedMessages store with "Queued" indicator

### Resume Phase
20. User clicks Resume button
21. System dispatches `resumeChat` action
22. System sends resume request to daemon
23. Daemon processes queued messages
24. Daemon appends queued messages to chat history
25. Daemon sends AI chat request with full context
26. Daemon transitions to Running state
27. System receives `chatResumed` action
28. System sets isPaused to false
29. System sets isStreaming to true
30. System clears queuedMessages store

## Expected Results
- Abort: AI request cancelled, partial response discarded, no message saved
- Message queue: Messages stored with "Queued" indicator
- Resume: Queued messages sent, no aborted AI call in message history

## Covered By

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) - `aborts during AI call and resumes without aborted message` (steps 2-30)

### UI Tests
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders abort button when streaming` (step 1)
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when paused` (step 20)
- [`Message.svelte`](frontend/src/lib/components/Message.svelte) - `removes streaming message on abort` (step 10)
```

### 4. `tests/cases/pause-abort/abort-during-tool-execution.md`

```markdown
# Test Case: Abort During Tool Execution

## Description
User aborts chat while tool calls are executing.

## Preconditions
- Chat exists and is selected
- AI chat call completed with tool calls
- Tool calls are executing (isStreaming = true, isPaused = false)

## Steps

### Abort Phase
1. User clicks Abort button
2. System dispatches `abortChat` action
3. System sends abort request to daemon
4. Daemon cancels execution of non-finished tool calls
5. Non-finished tool calls return "Aborted" error result
6. Finished tool calls keep their results
7. Daemon inserts all tool results (finished + aborted) into chat messages
8. Daemon transitions to Aborted state
9. Daemon emits `toolCallAborted` events for non-finished tools
10. System receives `toolCallAborted` actions
11. System updates tool call UI to show "Aborted" error
12. System sets isPaused to true
13. System sets isStreaming to false

### Message Queue Phase
14. User types message in input field
15. User clicks Send button
16. System dispatches `queueMessage` action
17. System sends queue request to daemon
18. Daemon stores message in message queue
19. System receives `messageQueued` action
20. System adds message to queuedMessages store with "Queued" indicator

### Resume Phase
21. User clicks Resume button
22. System dispatches `resumeChat` action
23. System sends resume request to daemon
24. Daemon processes queued messages
25. Daemon appends queued messages to chat history
26. Daemon sends AI chat request with full context
27. Daemon transitions to Running state
28. System receives `chatResumed` action
29. System sets isPaused to false
30. System sets isStreaming to true
31. System clears queuedMessages store

## Expected Results
- Abort: Non-finished tools return "Aborted" error, finished tools keep results, all results inserted
- Message queue: Messages stored with "Queued" indicator
- Resume: Queued messages sent, tool loop continues

## Covered By

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) - `aborts during tool execution and resumes with error results` (steps 2-31)

### UI Tests
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders abort button when streaming` (step 1)
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `renders resume button when paused` (step 21)
- [`ToolCall.test.ts`](frontend/src/tests/ui/ToolCall.test.ts) - `shows Aborted error for cancelled tool calls` (step 11)
```

### 5. `tests/cases/pause-abort/message-queue-during-pause-abort.md`

```markdown
# Test Case: Message Queue During Pause/Abort

## Description
User adds messages while chat is paused or aborted, messages are queued and sent on resume.

## Preconditions
- Chat exists and is selected
- Chat is paused or aborted (isPaused = true)

## Steps

### Queue First Message
1. User types first message in input field
2. User clicks Send button
3. System dispatches `queueMessage` action
4. System sends queue request to daemon
5. Daemon stores message in message queue
6. System receives `messageQueued` action
7. System adds message to queuedMessages store with "Queued" indicator

### Queue Second Message
8. User types second message in input field
9. User clicks Send button
10. System dispatches `queueMessage` action
11. System sends queue request to daemon
12. Daemon stores message in message queue
13. System receives `messageQueued` action
14. System adds message to queuedMessages store with "Queued" indicator

### Resume
15. User clicks Resume button
16. System dispatches `resumeChat` action
17. System sends resume request to daemon
18. Daemon processes queued messages in order
19. Daemon appends first queued message to chat history
20. Daemon appends second queued message to chat history
21. Daemon sends AI chat request with full context
22. Daemon transitions to Running state
23. System receives `chatResumed` action
24. System sets isPaused to false
25. System sets isStreaming to true
26. System clears queuedMessages store

## Expected Results
- Queue: Messages stored in order with "Queued" indicator
- Resume: Queued messages appended in order, AI call triggered with full context

## Covered By

### E2E Tests
- [`chat-state.test.ts`](frontend/src/tests/e2e/chat-state.test.ts) - `queues multiple messages during pause and sends on resume` (steps 3-26)

### UI Tests
- [`MessageInput.test.ts`](frontend/src/tests/ui/MessageInput.test.ts) - `allows message input when paused` (step 1)
- [`Message.test.ts`](frontend/src/tests/ui/Message.test.ts) - `shows Queued indicator for queued messages` (step 7)
```

## Files to Update

### 6. `tests/cases/chat-pause-resume.md`

**Deprecate old test case:**

Add a deprecation notice at the top of the file:

```markdown
> **DEPRECATED**: This test case has been superseded by the more detailed test cases in `tests/cases/pause-abort/`. See:
> - [`pause-during-ai-call.md`](pause-abort/pause-during-ai-call.md)
> - [`pause-during-tool-execution.md`](pause-abort/pause-during-tool-execution.md)
> - [`abort-during-ai-call.md`](pause-abort/abort-during-ai-call.md)
> - [`abort-during-tool-execution.md`](pause-abort/abort-during-tool-execution.md)
> - [`message-queue-during-pause-abort.md`](pause-abort/message-queue-during-pause-abort.md)
```

## Implementation Notes

1. **Step comment pattern**: Each test case follows the established pattern from the tests-grooming skill:
   - Steps are numbered sequentially across all phases (Pause, Message Queue, Resume)
   - Each step has a clear description matching what the system should do
   - Steps are grouped by phase for readability

2. **"Covered By" section**: Each test case includes references to:
   - E2E tests that cover the full flow (state logic)
   - UI tests that cover individual UI interactions
   - Step numbers are referenced to show which steps each test covers

3. **Test case structure**: Each file follows the same structure:
   - Description
   - Preconditions
   - Steps (grouped by phase)
   - Expected Results
   - Covered By

4. **Deprecation of old test case**: The old `chat-pause-resume.md` is deprecated but not deleted, to maintain backward compatibility with any existing references.

5. **Directory structure**: All new test cases are in `tests/cases/pause-abort/` to keep them organized and separate from other test cases.

## Dependencies

- This phase depends on: Phase 4 (Frontend Integration)
- This phase must be completed before: Phase 6 (E2E Test Implementation)
- This phase can be done in parallel with Phase 4 if test names are pre-defined
