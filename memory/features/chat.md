# Chat

## Purpose
Persistent conversational interface for direct AI interaction. Users create chats, send messages, receive streaming responses, and can edit/resend previous messages. All conversations persist across daemon restarts.

## Todo List
- AI can create and manage a task tracking list during multi-step operations
- Button appears near the role selector showing completed/total count (e.g., "3/5")
- Clicking the button expands a dropdown showing the full todo list
- Each item shows status icon and content:
  - ✓ (green) - Completed tasks (strikethrough text)
  - ◐ (blue) - In progress tasks (bold text)
  - ○ (gray) - Pending tasks
  - ✗ (red) - Discarded tasks (strikethrough, faded text)
- Real-time updates as the AI modifies the list
- Button only visible when a todo list exists
- Dropdown shows header with "Task List" title and completion stats
- Auto-updates via WebSocket events when AI calls `rhd_set_todo_list` tool
- Todo list is injected into AI context during tool loop iterations
- Tool contract is injected as system message on first message in chat

## Delete All Chats
- "Delete all chats" button at the bottom of the chat list sidebar
- Removes all chats and their messages at once
- Shows confirmation dialog before deleting
- Disabled when no chats exist

## How It Works

### Chat Lifecycle
1. User creates a new chat with a title
2. User selects a model from available models list
3. User sends messages; AI responds with streaming text
4. User can edit previous messages — this truncates the conversation after that point and resends to AI
5. User can abort streaming responses mid-generation
6. Reopening a chat restores the same model selection

### Model Selection
- Model selector dropdown appears under message input
- Available models fetched from daemon (real models only, not aliases)
- Selected model persists per chat — reopening chat shows same model
- Visual indicators in chat history show when model changes between messages
- Model indicators are visual-only, not sent to AI context

### Streaming Responses
- AI responses stream token-by-token
- WebSocket client shows animated dots indicator while streaming
- First chunk hides loader and shows assistant message with content
- Subsequent chunks append to content in real-time
- Stream finish removes animated dots, shows final message
- Empty chunks filtered on backend (not sent to WebSocket client)

### Smart Auto-Scrolling
- Message list auto-scrolls to bottom when new content arrives **only if user is at bottom**
- Tracks "at bottom" state with 30px threshold from bottom
- If user scrolls up, auto-scroll stops (respects user's scroll position)
- If user scrolls back to bottom, auto-scroll resumes
- New streaming session resets auto-scroll to enabled

### Thinking/Reasoning Content
- AI models with reasoning support (e.g., Qwen) emit thinking content
- Displayed in collapsible "Thinking" section (collapsed by default)
- **Collapsed preview**: Shows last 3 visual lines (rendered/wrapped) so user sees live streaming progress
- **Expanded state**: Auto-scrolls to bottom; continues auto-scroll while user stays at bottom; stops if user scrolls up; resumes if user scrolls back to bottom
- Visible in message history for assistant messages

### System Prompts
- System prompts displayed in collapsible "System Prompt" section (collapsed by default)
- Sent before user message in conversations

### MCP Tool Calls
- Tool calls displayed in `ToolCallMessage` component with MCP ID
- Collapsed by default, showing header with status icon, MCP ID, tool name
- Expanded view shows arguments and result sections
- Tool names are namespaced: `{mcp_id}/{tool_name}` format (e.g., `fs1/read_file`)
- Built-in tools (e.g., `rhd_set_flag`, `rhd_set_todo_list`) are not namespaced
- Failed tool calls show red X icon and red border styling

### Todo List
- AI can create and manage a task tracking list during multi-step operations
- Button appears near the role selector showing completed/total count (e.g., "3/5")
- Clicking the button expands a dropdown showing the full todo list
- Each item shows status icon and content:
  - ✓ (green) - Completed tasks (strikethrough text)
  - ◐ (blue) - In progress tasks (bold text)
  - ○ (gray) - Pending tasks
  - ✗ (red) - Discarded tasks (strikethrough, faded text)
- Real-time updates as the AI modifies the list
- Button only visible when a todo list exists
- Dropdown shows header with "Task List" title and completion stats
- Auto-updates via WebSocket events when AI calls `rhd_set_todo_list` tool

### Message Editing
- User can edit their own messages
- Editing truncates all messages after the edited message
- AI regenerates response from the edited point forward
- Model used for regeneration is the chat's active model

### Abort
- User can abort streaming response mid-generation
- Partial content may be displayed
- Error state shown with retry option

### Error Handling
- Network errors, API errors, and parse errors shown in UI
- Retry button resends the last message
- Error messages include context (model name, status code)
