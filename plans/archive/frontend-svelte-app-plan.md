# Svelte Frontend for RHD Daemon

## Goal

Create Svelte frontend application in `frontend/` folder that connects to RHD daemon WebSocket server and displays scenario execution information.

## Requirements

- Start with `nvm use && npm run start`
- `.nvmrc` with `v24.13.0`
- Connect to WebSocket server (default port 9876, override via `VITE_WS_PORT`)
- Two tabs: Scenarios and Chats
- Chats tab: "not implemented yet" screen
- Scenarios tab:
  - List of finished scenarios sorted by date (newest first)
  - Subscribe to active scenarios when tab opens
  - Real-time updates for active scenarios
  - Show current step and timer (seconds elapsed)
  - Show total input/output tokens and price in dollars
- Styling: CSS modules + utility classes (Tailwind-like approach)
- Cache finished scenarios in frontend store (persist across tab switches)
- Use `lastId` parameter in `getFinishedScenarios` to fetch only new scenarios

## Architecture

### Project Structure

```
frontend/
├── .nvmrc
├── package.json
├── vite.config.js
├── index.html
├── src/
│   ├── main.js
│   ├── App.svelte
│   ├── lib/
│   │   ├── ws.js              # WebSocket connection service
│   │   ├── stores.js          # Svelte stores for state management
│   │   └── utils.js           # Helper functions (formatting, etc.)
│   ├── components/
│   │   ├── TabNav.svelte      # Tab navigation component
│   │   ├── ScenariosTab.svelte
│   │   ├── ChatsTab.svelte
│   │   ├── ActiveScenario.svelte
│   │   └── FinishedScenario.svelte
│   └── styles/
│       ├── global.css
│       ├── utilities.css      # Tailwind-like utility classes
│       └── components/
│           ├── TabNav.module.css
│           ├── ScenariosTab.module.css
│           ├── ActiveScenario.module.css
│           └── FinishedScenario.module.css
```

### WebSocket Protocol

Based on [`rhd_api`](packages/rhd_api/src/lib.rs:138):

**Client → Server requests:**
```json
{"type": "subscribe", "id": "req-1"}
{"type": "getFinishedScenarios", "id": "req-2", "lastId": 0}
```

**Server → Client responses:**
```json
{
  "type": "response",
  "id": "req-1",
  "success": true,
  "data": {
    "activeExecutions": [
      {"id": 1, "scenarioName": "test", "startedAt": "2026-06-26T19:00:00Z"}
    ]
  }
}
{
  "type": "response",
  "id": "req-2",
  "success": true,
  "data": [
    {
      "id": 1,
      "scenario": "test",
      "started": "2026-06-26T19:00:00Z",
      "finished": "2026-06-26T19:00:30Z",
      "durationMs": 30000,
      "steps": [...],
      "tokens": {"promptTokens": 1000, "completionTokens": 500, "totalTokens": 1500},
      "cost": 0.0235
    }
  ]
}
```

**Server → Client events:**
```json
{
  "type": "event",
  "event": "scenariostarted",
  "data": {"id": 1, "name": "test", "startedAt": "2026-06-26T19:00:00Z"}
}
{
  "type": "event",
  "event": "stepstarted",
  "data": {"executionId": 1, "stepName": "build", "startedAt": "2026-06-26T19:00:05Z"}
}
{
  "type": "event",
  "event": "scenariofinished",
  "data": {
    "id": 1,
    "scenario": "test",
    "started": "2026-06-26T19:00:00Z",
    "finished": "2026-06-26T19:00:30Z",
    "durationMs": 30000,
    "steps": [...],
    "tokens": {"promptTokens": 1000, "completionTokens": 500, "totalTokens": 1500},
    "cost": 0.0235
  }
}
```

**IMPORTANT**: `scenariofinished` event data uses same `ScenarioMeta` format as `getFinishedScenarios` response items. This allows frontend to directly prepend finished event data to finished scenarios list without transformation.

### Daemon-Side Changes Required

#### 1. Align `scenariofinished` Event Format

Currently `EventData::ScenarioFinished` has different shape (`id`, `name`, `daemonTime`, `startedAt`, `finishedAt`) than `ScenarioMeta` (`scenario`, `started`, `finished`). Need align these:

- Modify [`ExecutionHandle::finished()`](packages/rhd_app/src/execution.rs:171) to:
  - Build `ScenarioMeta` struct from finished execution data
  - Write `meta.json` via `LogSink` (or return `ScenarioMeta` to caller who writes it)
  - Broadcast `ScenarioFinished` event with `ScenarioMeta` as data (serialized to JSON)

- Modify [`EventData::ScenarioFinished`](packages/rhd_api/src/lib.rs:38) variant to contain `ScenarioMeta` directly, or change serialization to match `ScenarioMeta` format.

- Ensure `ws.rs` broadcasts the `ScenarioMeta`-shaped data in `scenariofinished` events.

This way frontend receives identical format from both `getFinishedScenarios` response and `scenariofinished` event, enabling direct prepend to finished list.

#### 2. Add `lastId` Parameter to `getFinishedScenarios`

Modify [`WsRequest::GetFinishedScenarios`](packages/rhd_api/src/lib.rs:150) to accept optional `lastId` parameter:

```rust
#[serde(rename = "getFinishedScenarios")]
GetFinishedScenarios { id: String, last_id: Option<u64> },
```

Update [`handle_get_finished()`](packages/rhd_app/src/ws.rs:162) to:
- Read all finished scenarios from `meta.json` files
- Filter to return only scenarios with `id > lastId` (if `lastId` provided)
- Need extract execution ID from `ScenarioMeta` or log directory name

**Challenge**: `ScenarioMeta` doesn't currently contain execution ID. Options:
1. Add `execution_id: u64` field to `ScenarioMeta`
2. Parse ID from log directory name (format: `<scenarioName>-<timestamp>`)
3. Store ID in `meta.json` separately

**Recommended**: Add `execution_id` field to `ScenarioMeta` struct. This makes filtering straightforward and keeps data self-contained.

### Frontend Caching Strategy

**Store Structure:**
```javascript
// stores.js
export const finishedScenarios = writable([]);  // Array of ScenarioMeta
export const lastKnownId = writable(0);          // Highest ID seen so far
```

**Tab Lifecycle:**
1. **First visit to Scenarios tab:**
   - Send `getFinishedScenarios` with `lastId: 0`
   - Receive all finished scenarios
   - Populate `finishedScenarios` store
   - Set `lastKnownId` to max ID from response

2. **Subsequent visits:**
   - Store already contains cached scenarios
   - Send `getFinishedScenarios` with `lastId: lastKnownId`
   - Receive only new scenarios (if any)
   - Prepend new scenarios to `finishedScenarios` store
   - Update `lastKnownId`

3. **Real-time updates:**
   - On `scenariofinished` event: prepend to `finishedScenarios`, update `lastKnownId`

**Benefits:**
- No re-fetching entire list on tab switch
- Minimal data transfer (only new scenarios)
- Instant UI on return to tab

### State Management

Use Svelte stores for reactive state:

- `activeScenarios`: Map of execution ID → scenario data (from `subscribe` response + `scenariostarted`/`stepstarted` events)
- `finishedScenarios`: Array of `ScenarioMeta` objects (cached, from `getFinishedScenarios` response + `scenariofinished` events prepended)
- `lastKnownId`: Highest execution ID seen (for incremental fetching)
- `wsConnected`: Boolean connection status

### Component Responsibilities

**App.svelte:**
- Initialize WebSocket connection
- Render tab navigation
- Show active tab content

**TabNav.svelte:**
- Tab buttons (Scenarios, Chats)
- Active tab indicator

**ScenariosTab.svelte:**
- On mount:
  - If `finishedScenarios` empty: send `getFinishedScenarios` with `lastId: 0`
  - Else: send `getFinishedScenarios` with `lastId: lastKnownId` to fetch only new
  - Send `subscribe` request for active scenarios
- Render active scenarios list (if any)
- Render finished scenarios list (sorted by `started` descending)
- Handle real-time events:
  - `scenariostarted`: add to `activeScenarios` map
  - `stepstarted`: update current step in active scenario
  - `scenariofinished`: remove from `activeScenarios`, prepend `ScenarioMeta` to `finishedScenarios`, update `lastKnownId`

**ActiveScenario.svelte:**
- Display scenario name
- Show current step name (from latest `stepstarted` event)
- Show elapsed time (calculated from `startedAt`, updated every second via `setInterval`)
- Show token counts (input/output) — accumulated from `add_token_usage` calls (need track in store)
- Show cost in dollars

**FinishedScenario.svelte:**
- Display scenario name (`scenario` field)
- Show finished timestamp (`finished` field)
- Show duration (`durationMs`)
- Show token counts and cost

**ChatsTab.svelte:**
- Display "not implemented yet" message

### Styling Approach

**CSS Modules:**
- Component-specific styles in `*.module.css`
- Scoped class names to avoid conflicts

**Utility Classes:**
- `utilities.css` with Tailwind-like classes
- Examples: `.marginTopS`, `.paddingM`, `.flexRow`, `.textMuted`
- Import globally in `main.js`

**Design Tokens:**
- CSS custom properties for spacing, colors, fonts
- Consistent spacing scale (4px, 8px, 12px, 16px, 24px, 32px)

## Implementation Steps

### 1. Daemon-Side: Add `execution_id` to `ScenarioMeta`
- Add `execution_id: u64` field to `ScenarioMeta` struct in `rhd_api/src/lib.rs`
- Update `write_meta_json()` calls to include execution ID
- Update `read_finished_scenarios()` to parse execution ID

### 2. Daemon-Side: Align `scenariofinished` Event Format
- Modify `EventData::ScenarioFinished` to use `ScenarioMeta` shape or serialize as `ScenarioMeta`
- Update `ExecutionHandle::finished()` to build `ScenarioMeta` and include in event
- Ensure `meta.json` written before event broadcast (so `getFinishedScenarios` stays in sync)
- Update `ws.rs` if needed to broadcast correct format

### 3. Daemon-Side: Add `lastId` Parameter to `getFinishedScenarios`
- Update `WsRequest::GetFinishedScenarios` to accept `last_id: Option<u64>`
- Modify `handle_get_finished()` to filter scenarios by `execution_id > last_id`
- Return only scenarios newer than `lastId`

### 4. Project Setup
- Create `frontend/` directory
- Initialize Svelte + Vite project
- Add `.nvmrc` with `v24.13.0`
- Configure `package.json` with `start` script
- Add `vite.config.js` with WebSocket port env var

### 5. WebSocket Service
- Create `src/lib/ws.js`
- Implement connection logic with auto-reconnect
- Handle request/response correlation via `id` field
- Expose methods: `subscribe()`, `getFinishedScenarios(lastId)`
- Emit events for real-time updates

### 6. State Management
- Create `src/lib/stores.js`
- Define Svelte writable stores: `activeScenarios`, `finishedScenarios`, `lastKnownId`
- Add helper functions to update stores from WebSocket events

### 7. Utility Functions
- Create `src/lib/utils.js`
- Add date/time formatting functions
- Add token/cost formatting functions
- Add elapsed time calculation

### 8. Base Components
- Create `TabNav.svelte` with CSS module
- Create basic layout structure in `App.svelte`

### 9. Chats Tab
- Create `ChatsTab.svelte` with placeholder message

### 10. Scenarios Tab - Finished List
- Create `FinishedScenario.svelte` component
- Implement `ScenariosTab.svelte` to fetch and display finished scenarios
- Implement caching logic: check if store has data, fetch incrementally with `lastId`
- Sort by `started` date (newest first)

### 11. Scenarios Tab - Active Scenarios
- Create `ActiveScenario.svelte` component with timer
- Implement subscription logic in `ScenariosTab.svelte`
- Handle real-time events (`scenariostarted`, `stepstarted`, `scenariofinished`)
- Update active scenarios store
- On `scenariofinished`: remove from active, prepend to finished list, update `lastKnownId`

### 12. Styling
- Create `global.css` with base styles
- Create `utilities.css` with utility classes
- Add CSS modules for each component
- Define design tokens (spacing, colors)

### 13. Testing
- Start RHD daemon with WebSocket enabled
- Run frontend with `nvm use && npm run start`
- Verify WebSocket connection
- Test active scenario updates
- Verify finished scenarios list
- Test tab switching (verify caching works)
- Verify `scenariofinished` event prepends to list in correct format
- Verify incremental fetch with `lastId` parameter

## File Changes

### Modified Files (Daemon)
- `packages/rhd_api/src/lib.rs`: 
  - Add `execution_id` field to `ScenarioMeta`
  - Update `EventData::ScenarioFinished` to use `ScenarioMeta` shape
  - Add `last_id` parameter to `WsRequest::GetFinishedScenarios`
- `packages/rhd_app/src/execution.rs`: 
  - Build `ScenarioMeta` in `finished()`, include execution ID
  - Include `ScenarioMeta` in event data
- `packages/rhd_app/src/ws.rs`: 
  - Update `handle_get_finished()` to filter by `last_id`
  - Ensure correct broadcast format
- `packages/rhd_app/src/log.rs`: 
  - Update `write_meta_json()` to include execution ID
  - Update `read_finished_scenarios()` to parse execution ID

### New Files (Frontend)
- `frontend/.nvmrc`
- `frontend/package.json`
- `frontend/vite.config.js`
- `frontend/index.html`
- `frontend/src/main.js`
- `frontend/src/App.svelte`
- `frontend/src/lib/ws.js`
- `frontend/src/lib/stores.js`
- `frontend/src/lib/utils.js`
- `frontend/src/components/TabNav.svelte`
- `frontend/src/components/ScenariosTab.svelte`
- `frontend/src/components/ChatsTab.svelte`
- `frontend/src/components/ActiveScenario.svelte`
- `frontend/src/components/FinishedScenario.svelte`
- `frontend/src/styles/global.css`
- `frontend/src/styles/utilities.css`
- `frontend/src/styles/components/*.module.css`

## Risks

1. **WebSocket reconnection**: Need handle connection drops gracefully
2. **Timer accuracy**: Client-side timer may drift from server time
3. **Event ordering**: Ensure events processed in correct order
4. **Memory leaks**: Clean up WebSocket listeners on component unmount
5. **Daemon change compatibility**: Ensure `ScenarioMeta` format change doesn't break existing `meta.json` reading
6. **Execution ID extraction**: Need ensure execution ID properly propagated to `meta.json` writing

## Success Criteria

- Frontend starts with `nvm use && npm run start`
- Connects to WebSocket server on port 9876 (or `VITE_WS_PORT`)
- Scenarios tab shows finished scenarios sorted by date
- Active scenarios update in real-time with step name and timer
- Token counts and cost displayed correctly
- `scenariofinished` event prepends to finished list in same format as `getFinishedScenarios`
- Finished scenarios cached across tab switches
- `getFinishedScenarios` with `lastId` returns only new scenarios
- Chats tab shows "not implemented yet"
- No console errors or warnings
