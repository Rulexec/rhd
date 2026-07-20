# Phase 2: Add Step Comments to Existing UI Tests

## Overview

This phase adds step-by-step comments to existing UI tests. UI tests typically cover specific steps (e.g., rendering at a certain state), so comments should indicate which steps they cover.

**Scope:**
- Add step comments to `MessageInput.test.ts` for: chat-send-message (UI steps), chat-abort (UI steps), chat-pause-resume (UI steps)
- Add step comments to `ChatList.test.ts` for: chat-create (UI steps), chat-delete (UI steps)
- Add step comments to `Message.test.ts` for: chat-edit-message (UI steps), chat-streaming (UI steps)

**Out of scope:**
- E2E tests (covered in Phase 1)
- New test creation (covered in Phases 3-7)

## Files to Modify

### 1. `frontend/src/tests/ui/MessageInput.test.ts`

**Modifications:**
Add step comments to map each test to the UI steps it covers from the test cases.

#### Test: `renders send button when not streaming`

**Add step comments for chat-send-message.md:**

```typescript
// Covers chat-send-message.md steps 1-2 (UI rendering)
it('renders send button when not streaming', () => {
  // Step 1. User types message in input field (input field rendered)
  // Step 2. User clicks Send button (or presses Enter) - Send button visible
  render(MessageInput);
  expect(screen.getByText('Send')).toBeTruthy();
});
```

#### Test: `renders abort button when streaming`

**Add step comments for chat-abort.md:**

```typescript
// Covers chat-abort.md step 1 (UI rendering)
it('renders abort button when streaming', () => {
  // Step 1. User clicks Abort button - Abort button is visible when streaming
  isStreaming.set(true);
  render(MessageInput);
  expect(screen.getByText('Abort')).toBeTruthy();
});
```

#### Test: `renders pause button when streaming and not paused`

**Add step comments for chat-pause-resume.md:**

```typescript
// Covers chat-pause-resume.md pause step 1 (UI rendering)
it('renders pause button when streaming and not paused', () => {
  // Pause Step 1. User clicks Pause button - Pause button visible when streaming and not paused
  isStreaming.set(true);
  isPaused.set(false);
  render(MessageInput);
  expect(screen.getByText('Pause')).toBeTruthy();
});
```

#### Test: `renders resume button when paused`

**Add step comments for chat-pause-resume.md:**

```typescript
// Covers chat-pause-resume.md resume step 1 (UI rendering)
it('renders resume button when paused', () => {
  // Resume Step 1. User clicks Resume button - Resume button visible when paused
  isStreaming.set(true);
  isPaused.set(true);
  render(MessageInput);
  expect(screen.getByText('Resume')).toBeTruthy();
});
```

#### Test: `disables send button when no chat selected`

**Add step comments:**

```typescript
// Covers chat-send-message.md precondition (UI rendering)
it('disables send button when no chat selected', () => {
  // Preconditions: Chat exists and is selected - this test verifies the negative case
  currentChatId.set(null);
  render(MessageInput);
  const sendButton = screen.getByText('Send');
  expect(sendButton.hasAttribute('disabled')).toBe(true);
});
```

#### Test: `disables send button when no model selected`

**Add step comments:**

```typescript
// Covers chat-send-message.md precondition (UI rendering)
it('disables send button when no model selected', () => {
  // Preconditions: Model is selected - this test verifies the negative case
  selectedModel.set(null);
  render(MessageInput);
  const sendButton = screen.getByText('Send');
  expect(sendButton.hasAttribute('disabled')).toBe(true);
});
```

#### Test: `shows error message when streamError is set`

**Add step comments:**

```typescript
// Covers chat-send-message.md error handling (UI rendering)
it('shows error message when streamError is set', () => {
  // Error state rendering when stream fails
  streamError.set('Test error message');
  render(MessageInput);
  expect(screen.getByText('Test error message')).toBeTruthy();
  expect(screen.getByText('Retry')).toBeTruthy();
});
```

#### Test: `renders model selector with available models`

**Add step comments for chat-select-model.md:**

```typescript
// Covers chat-select-model.md steps 2, 4-5 (UI rendering)
it('renders model selector with available models', () => {
  // Step 2. System populates model selector dropdown
  // Step 4. User clicks model selector dropdown
  // Step 5. User selects a model
  render(MessageInput);
  const modelSelect = screen.getByLabelText('Model:');
  expect(modelSelect).toBeTruthy();
  const options = Array.from(modelSelect.querySelectorAll('option')).map(
    (o) => o.textContent
  );
  expect(options).toContain('model1');
  expect(options).toContain('model2');
});
```

### 2. `frontend/src/tests/ui/ChatList.test.ts`

**Modifications:**
Add step comments to map each test to the UI steps it covers from the test cases.

#### Test: `renders new chat button`

**Add step comments for chat-create.md:**

```typescript
// Covers chat-create.md steps 1-2 (UI rendering)
it('renders new chat button', () => {
  // Step 1. User clicks "+ New Chat" button - button is visible
  // Step 2. Dialog opens with title input field (dialog not tested here)
  render(ChatList);
  expect(screen.getByText('+ New Chat')).toBeTruthy();
});
```

#### Test: `renders chat list items`

**Add step comments:**

```typescript
// Covers chat-create.md expected results (UI rendering)
it('renders chat list items', () => {
  // Expected Results: New chat appears in chat list
  chats.set([
    { id: 1, title: 'Chat 1', createdAt: '', updatedAt: '', activeModel: null },
    { id: 2, title: 'Chat 2', createdAt: '', updatedAt: '', activeModel: null },
  ]);
  render(ChatList);
  expect(screen.getByText('Chat 1')).toBeTruthy();
  expect(screen.getByText('Chat 2')).toBeTruthy();
});
```

#### Test: `highlights selected chat`

**Add step comments for chat-create.md:**

```typescript
// Covers chat-create.md step 9 (UI rendering)
it('highlights selected chat', () => {
  // Step 9. System sets currentChatId to new chat - UI reflects selection
  chats.set([
    { id: 1, title: 'Chat 1', createdAt: '', updatedAt: '', activeModel: null },
    { id: 2, title: 'Chat 2', createdAt: '', updatedAt: '', activeModel: null },
  ]);
  currentChatId.set(1);
  render(ChatList);
  const chat1Element = screen.getByText('Chat 1').closest('.chat-item');
  expect(chat1Element?.classList.contains('selected')).toBe(true);
});
```

#### Test: `renders delete button for each chat`

**Add step comments for chat-delete.md:**

```typescript
// Covers chat-delete.md steps 1-2 (UI rendering)
it('renders delete button for each chat', () => {
  // Step 1. User clicks delete button (×) on chat item - button is visible
  // Step 2. System shows confirmation dialog (dialog not tested here)
  chats.set([
    { id: 1, title: 'Chat 1', createdAt: '', updatedAt: '', activeModel: null },
  ]);
  render(ChatList);
  const deleteButtons = screen.getAllByLabelText('Delete chat');
  expect(deleteButtons.length).toBe(1);
});
```

### 3. `frontend/src/tests/ui/Message.test.ts`

**Modifications:**
Add step comments to map each test to the UI steps it covers from the test cases.

#### Test: `renders user message content`

**Add step comments:**

```typescript
// Covers chat-send-message.md expected results (UI rendering)
it('renders user message content', () => {
  // Expected Results: User message appears in message list
  const message: ChatMessage = {
    id: 1,
    chatId: 1,
    role: 'user',
    content: 'Hello, AI!',
    createdAt: '',
    model: 'model1',
  };
  render(Message, { props: { message } });
  expect(screen.getByText('Hello, AI!')).toBeTruthy();
});
```

#### Test: `renders assistant message content`

**Add step comments:**

```typescript
// Covers chat-send-message.md expected results (UI rendering)
it('renders assistant message content', () => {
  // Expected Results: Assistant message content displayed
  const message: ChatMessage = {
    id: 2,
    chatId: 1,
    role: 'assistant',
    content: 'Hello, user!',
    createdAt: '',
    model: 'model1',
  };
  render(Message, { props: { message } });
  expect(screen.getByText('Hello, user!')).toBeTruthy();
});
```

#### Test: `renders edit button for user messages`

**Add step comments for chat-edit-message.md:**

```typescript
// Covers chat-edit-message.md steps 1-2 (UI rendering)
it('renders edit button for user messages', () => {
  // Step 1. User clicks edit button on user message - button is visible
  // Step 2. System enters edit mode for that message (edit mode not tested here)
  const message: ChatMessage = {
    id: 1,
    chatId: 1,
    role: 'user',
    content: 'Hello, AI!',
    createdAt: '',
    model: 'model1',
  };
  render(Message, { props: { message } });
  const editButton = screen.getByLabelText('Edit message');
  expect(editButton).toBeTruthy();
});
```

#### Test: `does not render edit button for assistant messages`

**Add step comments:**

```typescript
// Covers chat-edit-message.md precondition (UI rendering)
it('does not render edit button for assistant messages', () => {
  // Only user messages can be edited
  const message: ChatMessage = {
    id: 2,
    chatId: 1,
    role: 'assistant',
    content: 'Hello, user!',
    createdAt: '',
    model: 'model1',
  };
  render(Message, { props: { message } });
  const editButton = screen.queryByLabelText('Edit message');
  expect(editButton).toBeNull();
});
```

#### Test: `shows streaming dots for streaming message`

**Add step comments for chat-streaming.md:**

```typescript
// Covers chat-streaming.md step 4 (UI rendering)
it('shows streaming dots for streaming message', () => {
  // Step 4. System displays animated dots indicator while streaming
  const message: ChatMessage = {
    id: 'temp-123',
    chatId: 1,
    role: 'assistant',
    content: 'Streaming...',
    createdAt: '',
    model: 'model1',
  };
  streamingMessageId.set('temp-123');
  render(Message, { props: { message } });
  const dots = document.querySelector('.streaming-dots');
  expect(dots).toBeTruthy();
});
```

#### Test: `renders thinking content when present`

**Add step comments for chat-mcp-tools.md:**

```typescript
// Covers chat-mcp-tools.md step 7 (UI rendering)
it('renders thinking content when present', () => {
  // Step 7. AI streams thinking content via `ThinkingChunk` events - UI displays thinking
  const message: ChatMessage = {
    id: 2,
    chatId: 1,
    role: 'assistant',
    content: 'Response',
    createdAt: '',
    model: 'model1',
    thinkingContent: 'Thinking process...',
  };
  render(Message, { props: { message } });
  expect(screen.getByText('Thinking')).toBeTruthy();
});
```

#### Test: `renders system message with collapsible header`

**Add step comments:**

```typescript
// Covers system message rendering (UI rendering)
it('renders system message with collapsible header', () => {
  // System messages are displayed with collapsible header
  const message: ChatMessage = {
    id: 3,
    chatId: 1,
    role: 'system',
    content: 'System prompt content',
    createdAt: '',
    model: null,
  };
  render(Message, { props: { message } });
  expect(screen.getByText('System Prompt')).toBeTruthy();
});
```

## Implementation Notes

1. **Comment Format**: Use `// Covers test-case.md steps X-Y (UI rendering)` format
2. **UI vs State Logic**: Clearly indicate these are UI rendering tests, not state logic tests
3. **Preconditions**: Mark tests that verify preconditions (like "no chat selected")
4. **Expected Results**: Mark tests that verify expected results from test cases
5. **No Code Changes**: Only add comments, do not modify test logic

## Dependencies

- This phase depends on: None (can be done in parallel with Phase 1)
- This phase must be completed before: Phase 8 (validation)
- Phase 1 can be done in parallel with this phase

## Success Criteria

- All existing UI tests have step comments
- Comments clearly map to test case steps
- Comments indicate which steps are covered by UI rendering vs state logic
- No test logic is modified
