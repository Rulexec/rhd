# Frontend UI

## Purpose
Svelte-based web interface for monitoring scenario execution and interacting with AI chat. Connects to daemon via WebSocket for real-time updates.

## State Export/Import
For debugging and testing, the frontend exposes global functions on `window`:
- `window.__exportState()` — returns a JSON-serializable object containing all frontend state (stores, router hash)
- `window.__importState(state)` — restores all stores and URL hash from an exported state object
- Round-trip is idempotent: exporting after import yields the same state (excluding volatile/derived values)
- Works from browser console for debugging
- Captures: scenario stores, chat stores, project stores, router hash

## Architecture
- **Framework:** Svelte 5 with TypeScript
- **Build tool:** Vite
- **State management:** Svelte stores (writable, derived)
- **Communication:** WebSocket (JSON protocol)
- **Styling:** CSS modules + utility classes (Tailwind-like)
- **Runtime validation:** Zod schemas for WebSocket messages

## Tabs

### Scenarios Tab
Displays scenario execution status in three sections:

1. **Paused scenarios** (orange badge) — scenarios paused due to AI errors (when `neverFail` enabled)
   - Shows error message, current step name
   - Model selector dropdown for retry
   - Retry button (continues execution with same or different model)
   - Abort button (emits error to `rhd run` client)

2. **Active scenarios** (blue badge) — currently executing scenarios
   - Shows scenario name, current step name, elapsed time (live timer)
   - Token counts (input/output) and cost in dollars
   - Abort button to stop execution

3. **Finished scenarios** (green/red/orange badge) — completed scenarios sorted by date (newest first)
   - Shows scenario name, status, finished timestamp, duration, tokens, cost
   - Cached in frontend store (persist across tab switches)
   - Incremental fetch via `lastId` parameter (only new scenarios)

### Chats Tab
Two-column layout:

1. **Chat list sidebar**
   - "New Chat" button (opens modal dialog)
   - List of chats with title and timestamp
   - Delete button on each chat
   - "Delete all chats" button at bottom (with confirmation)
   - Click to select chat

### New Chat Dialog
- Modal dialog using native HTML `<dialog>` element
- Centered on screen with fade overlay (backdrop)
- Input field for chat title (autofocused)
- Create button (disabled when input empty)
- Cancel button
- Escape key closes dialog
- Replaces browser's native `prompt()` for better UX

2. **Chat view** (when chat selected)
   - Header with chat title
   - Message list (scrollable, smart auto-scroll on new content)
   - Model selector dropdown under input
   - Message input with send/abort/retry
   - Streaming message display with animated dots indicator
   - Edit button on user messages (truncates and resends)
   - MCP tool call display with collapsible details (ToolCallMessage component)
   - Markdown rendering for assistant messages (enabled by default, per-message toggle)

### Markdown Rendering
- Assistant messages render markdown by default (headings, lists, code blocks, links, tables, etc.)
- Per-message toggle button in top-right corner (MD/Raw) to switch between markdown and raw text
- Applies to both message content and thinking content
- User messages and system messages remain plain text
- Uses `marked` for parsing and `dompurify` for XSS protection
- Toggle state is per-session only (not persisted)

## Real-Time Updates
- WebSocket connection to daemon (default port 9876, configurable via `VITE_WS_PORT`)
- Auto-reconnect on connection drop
- Ping/pong for liveness tracking (2s interval, 5s timeout)
- Events: `scenarioStarted`, `stepStarted`, `scenarioFinished`, `scenarioPaused`, `scenarioResumed`, `chatStreamChunk`, `chatStreamFinished`, `chatMessageAdded`, `chatToolCallStarted`, `chatToolCallCompleted`, `projectAttached`, `projectDetached`, `projectMcpStatusChanged`, etc.

## URL Routing
Hash-based routing persists active tab and selected chat:
- `#scenarios` — Scenarios tab (default)
- `#chats` — Chats tab, no chat selected
- `#chats/123` — Chats tab, chat ID 123 selected
- Page reload restores state from URL hash

## Notifications
- Browser notifications on scenario pause (requires permission)
- Click notification focuses browser tab
- Desktop notifications from daemon (via `terminal-notifier` or `osascript` on macOS)

## Date/Time Formatting
- ISO-like format: `YYYY-MM-DD HH:MM:SS` (local time, 24h)
- Backend timestamps remain UTC ISO 8601
- Duration formatted as human-readable string
- Cost formatted as dollars with 4 decimal places

## Key Files
- Entry point: `frontend/src/main.ts`
- App component: `frontend/src/App.svelte`
- Router: `frontend/src/lib/router.ts`
- WebSocket client: `frontend/src/lib/ws.ts`
- Scenario stores: `frontend/src/lib/stores.ts`
- Chat stores: `frontend/src/lib/chatStores.ts`
- Project stores: `frontend/src/lib/projectStores.ts`
- Chat WebSocket: `frontend/src/lib/chatWs.ts`
- State export/import: `frontend/src/lib/stateExport.ts`
- Markdown renderer: `frontend/src/lib/markdown.ts`
- Types & Zod schemas: `frontend/src/lib/types/index.ts`, `frontend/src/lib/types/ws.ts`
- Components: `frontend/src/components/*.svelte` (includes `ToolCallMessage.svelte`, `ProjectsPanel.svelte`, `McpStatusDrawer.svelte`)
- Styles: `frontend/src/styles/global.css`, `frontend/src/styles/utilities.css`
