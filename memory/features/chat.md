# Chat

## Purpose
Persistent conversational interface for direct AI interaction. Users create chats, send messages, receive streaming responses, and can edit/resend previous messages. All conversations persist across daemon restarts.

## Frontend UI

### Chat List
- Sidebar displays all chats sorted by `updatedAt` DESC
- Real-time updates: new chats appear immediately, deleted chats disappear
- Each chat shows title, relative timestamp ("5m ago", "2h ago"), and up to 2 tags
- "+ New Chat" button creates chat with auto-generated title (format: `YYYY-MM-DD HH:mm`)
- "Delete All Chats" button at bottom (hidden when no chats exist)
- Confirmation modal before deleting all chats (closes on outside click or Escape)
- Loading state while fetching initial chat list
- Empty state when no chats exist

### Tab Navigation
- Two tabs: "Chats" and "Plugins"
- Active tab highlighted with bottom border
- Tab state preserved when switching between tabs

### Chat View
- Displays messages for selected chat in chronological order
- Chat header shows title and tags; header tags update reactively when plugins add tags to the current chat
- Loading state while fetching chat data
- Empty state when no messages or no chat selected
- Error state with dismissible error message

### Chat Tag Additions
- "+" button next to the tags in the chat header opens a tag input dropdown
- Dropdown contains a text input and tag suggestions aggregated from all existing chats (unique, sorted)
- Typing filters the suggestions; already-applied tags are excluded from the list
- Clicking a suggestion adds that tag; pressing Enter creates a new tag from the typed text; Escape closes the dropdown
- Added tags appear immediately in the chat header and the chat list (via `chatUpdated` events)

### Chat View Tabs
- The chat view has two tabs: "Messages" and "Tools"
- Messages tab shows the conversation (default)
- Tools tab lists the tools registered for the current chat: tool name, description, and the plugin ID that registered it
- Tools list updates in real time via `toolsUpdated` events (e.g., when a plugin registers or removes a tool)

### Message Display
- Messages show role badge (user/assistant/system), timestamp, and tags
- Markdown rendering enabled by default
- Toggle button switches between Markdown and plain text view
- Reasoning content displayed in collapsible section (collapsed by default)
- Collapsed preview shows last 3 lines with scroll-to-bottom
- Expand/collapse button toggles full content view
- Queue messages displayed with `opacity: 0.8` to distinguish from regular messages

### Message Input
- Multi-line textarea with auto-resize (grows with content, max 200px)
- Enter key sends message, Shift+Enter adds new line
- Send button disabled when input is empty, no chat selected, or WebSocket disconnected
- Input clears after sending
- Inline error message shown if send fails
- Hint text shows "Select a chat to send messages" or "Not connected to server"

### Plugins Tab
- Displays list of all registered plugins sorted alphabetically by pluginId
- Each plugin shows pluginId and active/inactive status
- Status indicator: green dot for active, gray dot for inactive
- Header shows active/total count (e.g., "2 / 3 active")
- Empty state: "No plugins registered" with helpful description
- Real-time updates when plugins register/remove/update

### Connection Status
- Fixed banner at top of page when not connected
- Shows connection state: "Connected" (green), "Disconnected" (red), "Connecting" (yellow)
- "Reconnect" button when disconnected
- Error message displayed when connection fails
- Banner auto-hides when connected

### Responsive Design
- Mobile layout (< 768px): chat list moves to top, chat view to bottom
- Font sizes reduced for mobile (< 480px)
- Custom scrollbar styling
- Focus states for keyboard navigation

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

## Queue Messages
- Separate message queue alongside regular messages
- Queue messages wait to be processed by plugins (e.g., AI completions)
- CRUD operations: add, update, delete, list queue messages
- Queue messages have the same structure as regular messages (role, content, model, etc.)
- Tags can be applied to queue messages
- Real-time events: queueMessageAdded, queueMessageUpdated, queueMessageDeleted
- Plugins can monitor queue and process messages when ready
- Queue is per-chat, independent from message history

## Tools Management
- Chat-level tool management for plugins
- Plugins can register tools for a specific chat
- Tools have: name, description, parameters (JSON schema)
- Each tool is associated with the plugin that registered it
- CRUD operations: addTools, removeTools, getTools
- Tools are scoped to a chat, not global
- Real-time events: toolsUpdated when tools change
- Plugins validate tool registration (must be registered for the connection)

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

### Server-Side Streaming (Plugin-Driven)

AI responses are streamed through the chat server via a plugin-driven streaming architecture:

1. **Stream Creation**: When `rhd_plugin_ai_completions` starts an AI request, it creates a message with `isStreaming: true, isFinished: false` before the request begins.
2. **Stream Push**: As the AI provider returns tokens, the plugin pushes deltas (reasoning content, content, tool calls) to the server-side stream via `streamPush`.
3. **Stream Subscription**: The frontend detects messages with `isStreaming: true` and calls `streamSubscribe` to receive the current accumulated content and future chunks.
4. **Stream Finish**: When the AI response is complete, the plugin calls `streamFinish` and updates the message with `isStreaming: false, isFinished: true` and the final content.

**Stream Events**:
- `streamChunk`: Broadcast to chat subscribers when new content is pushed. Contains `type` (reasoningDelta/contentDelta/toolCallDelta) and the delta content.
- `streamFinished`: Broadcast when a stream completes.

**Stream Methods**:
- `streamPush`: Called by plugins to push deltas to a stream.
- `streamSubscribe`: Called by frontend to get current state and subscribe to future chunks.
- `streamFinish`: Called by plugins to finalize a stream.

**Message Flags**:
- `isStreaming: true` — Message is currently being streamed.
- `isFinished: false` — Message content is not yet final.
- Default values: `isStreaming: false, isFinished: true` (backward compatible with non-streaming messages).

**Frontend Behavior**:
- The frontend automatically subscribes to the active stream as soon as a streaming message appears and unsubscribes when it disappears (reactive, store-driven; works across chat switches and mid-stream reopen)
- The "Generating response..." placeholder is shown only while both content and reasoning content are empty

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
- System-role messages are collapsed by default behind a "System Message" toggle (▼/▶), following the same pattern as the reasoning section
- Non-system messages (user, assistant) are unaffected
- Sent before user message in conversations

### MCP Tool Calls
- Tool calls displayed in `ToolCallMessage` component
- Collapsed by default, showing header with status icon and tool name
- Expanded view shows arguments and result sections
- Tools from the MCP plugin carry their server prefix: `<name>:<tool>` format (e.g., `filesystem:read_file`) — see [mcp-plugin.md](mcp-plugin.md)
- Built-in plugin tools (e.g., `rhd_set_todo_list`) are not namespaced
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
