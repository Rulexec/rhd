# Frontend UI Testing Plan

## Goal

Enable end-to-end UI testing where:
- `rhd_test frontend` starts daemon with WebSocket + mock AI server + control server
- Vitest tests with jsdom interact with frontend components
- Tests configure mock AI responses via control server HTTP API

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    rhd_test frontend                         │
├─────────────────────────────────────────────────────────────┤
│  1. Mock AI Server (existing, extended)                     │
│     - POST /v1/chat/completions                             │
│     - Configurable responses via shared state               │
│                                                             │
│  2. rhd daemon (spawned with --ws-port)                     │
│     - WebSocket server for frontend                         │
│     - Uses mock AI server as backend                        │
│                                                             │
│  3. Control HTTP Server (new)                               │
│     - POST /mock-response - set AI response content         │
│     - GET /status - check daemon ready                      │
│     - GET /requests - get recorded AI requests              │
│                                                             │
│  4. Vitest Test Runner (jsdom environment)                  │
│     - Tests call control server to configure mocks          │
│     - Tests mount Svelte components with jsdom              │
│     - Tests assert UI state and behavior                    │
└─────────────────────────────────────────────────────────────┘
```

## Implementation Steps

### Phase 1: Prepare test models config

1. **Create test models directory** (`test_e2e/frontend_models/`)
   - Create `test-model.yaml` with placeholder for API base URL
   - Example:
     ```yaml
     baseUrl: "http://localhost:${E2E_MODEL_PORT}/v1"
     apiKey: "test-key"
     ```
   - This file will be edited by rhd_test to inject actual mock server port

### Phase 2: Extend rhd_test with frontend subcommand

2. **Add `frontend` subcommand to args.rs**
   - Add `--ws-port` (default: random available port)
   - Add `--control-port` (default: random available port)
   - No `--frontend-port` needed (jsdom runs in Node.js, no browser server)

3. **Create control server module** (`packages/rhd_test/src/control_server.rs`)
   - Axum HTTP server with endpoints:
     - `POST /mock-response` - body: `{"content": "..."}` - sets mock AI response
     - `GET /status` - returns daemon status
     - `GET /requests` - returns recorded AI requests
   - Shares state with mock AI server (response, requests)

4. **Create frontend test module** (`packages/rhd_test/src/frontend_test.rs`)
   - Start mock AI server on random port
   - Edit test model files to set `baseUrl` with actual mock server port
   - Start control server
   - Spawn daemon with `--ws-port`, `--models-dir test_e2e/frontend_models`
   - Wait for daemon ready (WebSocket listening)
   - Run vitest tests (tests will connect to daemon WebSocket)
   - Cleanup (kill daemon, servers, restore model files)

5. **Update mock_server.rs**
   - Keep existing functionality
   - Ensure shared state works for configurable responses

### Phase 3: Frontend test setup

6. **Install test dependencies** (`frontend/package.json`)
   ```json
   {
     "devDependencies": {
       "vitest": "^1.0.0",
       "@testing-library/svelte": "^4.0.0",
       "@testing-library/jest-dom": "^6.0.0",
       "jsdom": "^24.0.0"
     }
   }
   ```

7. **Configure Vitest** (`frontend/vite.config.ts`)
   ```typescript
   export default defineConfig({
     plugins: [svelte()],
     test: {
       environment: 'jsdom',
       globals: true,
       setupFiles: ['./src/tests/setup.ts'],
       include: ['src/**/*.{test,spec}.{js,ts}']
     }
   });
   ```

8. **Create test setup** (`frontend/src/tests/setup.ts`)
   - Import `@testing-library/jest-dom`
   - Configure WebSocket URL from environment variable (passed from rhd_test)

9. **Create test utilities** (`frontend/src/tests/testUtils.ts`)
   - `configureMock(content)` - call control server to set AI response
   - `getRecordedRequests()` - fetch from control server
   - `waitForWebSocket()` - wait for frontend to connect to daemon
   - Helper to mount components with proper store initialization

### Phase 4: WebSocket handling in jsdom

10. **WebSocket polyfill/mock strategy**
    - jsdom has basic WebSocket support
    - Frontend code uses native WebSocket API
    - Tests will connect to real daemon WebSocket (localhost:ws-port)
    - No mocking needed - real WebSocket connection in jsdom

11. **Update frontend WebSocket client** (`frontend/src/lib/ws.ts`)
    - Ensure WebSocket URL configurable via environment variable
    - Default to `VITE_WS_PORT` or passed port
    - Tests will set `VITE_WS_PORT` to daemon's ws-port

### Phase 5: First test case

12. **Write chat creation test** (`frontend/src/tests/chat.test.ts`)
    ```typescript
    import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
    import ChatsTab from '../components/ChatsTab.svelte';
    import { configureMock, waitForWebSocket } from './testUtils';

    describe('Chat UI', () => {
      beforeEach(async () => {
        // Wait for WebSocket connection
        await waitForWebSocket();
      });

      it('creates new chat with model pre-selected', async () => {
        // Render chats tab
        render(ChatsTab);
        
        // Click "New Chat" button
        const newChatButton = screen.getByText('New Chat');
        await fireEvent.click(newChatButton);
        
        // Enter chat name in prompt/dialog
        // (implementation depends on how new chat dialog works)
        
        // Wait for chat to open
        await waitFor(() => {
          expect(screen.getByText('Test Chat')).toBeInTheDocument();
        });
        
        // Verify model select has 'test-model' pre-selected
        const modelSelect = screen.getByLabelText('Model:');
        expect(modelSelect).toHaveValue('test-model');
      });
    });
    ```

## Test Flow Example

```
1. Prepare test models:
   - test_e2e/frontend_models/test-model.yaml exists with placeholder

2. Run: cargo run -p rhd_test -- frontend

3. rhd_test starts:
   - Mock AI server on random port (e.g., 12345)
   - Edit test-model.yaml: set baseUrl to http://localhost:12345/v1
   - Control server on random port (e.g., 8080)
   - rhd daemon with --ws-port 9876 --models-dir test_e2e/frontend_models
   
4. Vitest starts (in same process or spawned):
   - VITE_WS_PORT=9876 environment variable set
   - Tests run in jsdom environment
   - Frontend components connect to ws://localhost:9876
   
5. Test executes:
   - POST http://localhost:8080/mock-response {"content": "AI response"}
   - Render ChatsTab component
   - Click "New Chat"
   - Enter "Test Chat"
   - Assert model select shows "test-model"
   
6. Cleanup:
   - Kill daemon
   - Kill servers
   - Restore test-model.yaml to original state
```

## File Changes

### New files:
- `test_e2e/frontend_models/test-model.yaml` - Test model config with placeholder
- `packages/rhd_test/src/control_server.rs` - Control HTTP server
- `packages/rhd_test/src/frontend_test.rs` - Frontend test orchestration
- `frontend/src/tests/setup.ts` - Vitest setup
- `frontend/src/tests/testUtils.ts` - Test utilities
- `frontend/src/tests/chat.test.ts` - First test case

### Modified files:
- `packages/rhd_test/src/main.rs` - Add frontend subcommand routing
- `packages/rhd_test/src/args.rs` - Add frontend subcommand args
- `frontend/package.json` - Add test dependencies
- `frontend/vite.config.ts` - Add Vitest config
- `frontend/src/lib/ws.ts` - Ensure WebSocket URL configurable

## Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| WebSocket not working in jsdom | jsdom supports WebSocket natively; test connection early |
| Port conflicts | Use random available ports by default |
| Slow test execution | jsdom is faster than real browser |
| Flaky tests | Add proper wait conditions, avoid fixed timeouts |
| Daemon startup race | Wait for "listening on" message before tests |
| Model file editing | Use temp directory or restore after tests |

## Success Criteria

- `cargo run -p rhd_test -- frontend` starts all services
- Tests can configure mock AI responses via control server
- First test passes: create chat, verify model pre-selection
- Tests run reliably in CI
- Test execution time < 10 seconds for basic tests

## Future Enhancements

- Add more test cases (send message, streaming, abort)
- Add component-level unit tests (faster, no daemon needed)
- Add test coverage reporting
- Support parallel test execution
