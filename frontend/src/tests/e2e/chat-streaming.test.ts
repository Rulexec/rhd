import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { spawn, ChildProcess } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { get } from 'svelte/store';
import { waitForWebSocket, setControlPort, setWsPort, configureMock, setAutoStream, emitStreamChunk, finishStream, waitForStreamReady } from '../testUtils';
import { setWsPort as setWsWsPort, connectWebSocket } from '../../lib/ws';
import { wsConnected } from '../../lib/stores';
import { messages, isStreaming, streamingMessageId } from '../../lib/chatStores';
import ChatsTab from '../../components/ChatsTab.svelte';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

let rhdProcess: ChildProcess | null = null;

beforeAll(async () => {
  const workspaceRoot = resolve(__dirname, '../../../..');
  const rhdTestBin = resolve(workspaceRoot, 'target/debug/rhd_test');

  rhdProcess = spawn(rhdTestBin, ['frontend'], {
    cwd: workspaceRoot,
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  let stdoutBuffer = '';
  
  const portsPromise = new Promise<{ controlPort: number; wsPort: number }>((resolve) => {
    rhdProcess!.stdout?.on('data', (data: Buffer) => {
      const text = data.toString();
      stdoutBuffer += text;
      console.log(`[rhd_test] ${text}`);
      
      const controlMatch = stdoutBuffer.match(/Control server started on port (\d+)/);
      const wsMatch = stdoutBuffer.match(/WebSocket server started on port (\d+)/);
      
      if (controlMatch && wsMatch) {
        resolve({
          controlPort: parseInt(controlMatch[1], 10),
          wsPort: parseInt(wsMatch[1], 10),
        });
      }
    });
  });

  rhdProcess.stderr?.on('data', (data: Buffer) => {
    console.error(`[rhd_test] ${data.toString()}`);
  });

  const { controlPort, wsPort } = await portsPromise;
  setControlPort(controlPort);
  setWsPort(wsPort);
  setWsWsPort(wsPort);
  connectWebSocket();
  
  await waitForWebSocket();
}, 30000);

afterAll(() => {
  if (rhdProcess) {
    rhdProcess.kill('SIGTERM');
    rhdProcess = null;
  }
});

describe('Chat streaming', () => {
  it('renders streaming chunks in real-time with animated indicator', async () => {
    if (!window.prompt) {
      (window as any).prompt = () => null;
    }
    vi.spyOn(window, 'prompt').mockReturnValue('Streaming Test Chat');

    await waitFor(
      () => {
        const wsState = get(wsConnected);
        if (!wsState) {
          throw new Error('WebSocket not connected');
        }
      },
      { timeout: 5000 }
    );

    render(ChatsTab);

    // Create new chat
    const newChatButton = screen.getByText('+ New Chat');
    await fireEvent.click(newChatButton);

    // Wait for model select to be available and auto-selected
    await waitFor(
      () => {
        const modelSelect = screen.getByLabelText('Model:') as HTMLSelectElement;
        if (!modelSelect || modelSelect.options.length === 0) {
          throw new Error('Model select not ready');
        }
        if (!modelSelect.value) {
          throw new Error('Model not auto-selected yet');
        }
      },
      { timeout: 5000 }
    );

    // Configure mock (required to initialize mock server, even though we'll control streaming manually)
    await configureMock('');
    
    // Disable auto-stream mode so we can control chunks manually
    await setAutoStream(false);

    // Send message
    const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
    await fireEvent.input(textarea, { target: { value: 'Hello, AI!' } });

    const sendButton = screen.getByText('Send');
    await fireEvent.click(sendButton);

    // Verify loader shown initially (before first chunk)
    await waitFor(
      () => {
        const streaming = get(isStreaming);
        if (!streaming) {
          throw new Error('Expected streaming to be true');
        }
        const tempId = get(streamingMessageId);
        if (tempId !== null) {
          throw new Error('Expected no streaming message yet (loader should be shown)');
        }
        // Check that loading dots are visible
        const loader = document.querySelector('.loading');
        if (!loader) {
          throw new Error('Expected loader to be visible');
        }
      },
      { timeout: 5000 }
    );

    // Wait for SSE stream to be ready
    await waitForStreamReady();
    
    // Small delay to ensure SSE connection is fully established
    await new Promise(resolve => setTimeout(resolve, 1000));
    
    // Emit first chunk
    await emitStreamChunk("You're absolutely right");
    
    await waitFor(
      () => {
        // Check loader hidden
        const loader = document.querySelector('.loading');
        if (loader) {
          throw new Error('Expected loader to be hidden after first chunk');
        }

        // Check streaming message created
        const tempId = get(streamingMessageId);
        if (!tempId) {
          throw new Error('Expected streaming message to be created');
        }

        // Check assistant message content
        const currentMessages = get(messages);
        const assistantMsg = currentMessages.find(m => m.role === 'assistant');
        if (!assistantMsg || assistantMsg.content !== "You're absolutely right") {
          throw new Error(`Expected first chunk content, got: ${assistantMsg?.content}`);
        }

        // Check animated dots present
        const dots = document.querySelector('.streaming-dots');
        if (!dots) {
          throw new Error('Expected streaming indicator (animated dots)');
        }
      },
      { timeout: 5000 }
    );

    // Emit second chunk
    await emitStreamChunk("! I should do it like");
    
    await waitFor(
      () => {
        const currentMessages = get(messages);
        const assistantMsg = currentMessages.find(m => m.role === 'assistant');
        if (!assistantMsg || assistantMsg.content !== "You're absolutely right! I should do it like") {
          throw new Error(`Expected combined content, got: ${assistantMsg?.content}`);
        }

        // Animated dots should still be present
        const dots = document.querySelector('.streaming-dots');
        if (!dots) {
          throw new Error('Expected streaming indicator to still be present');
        }
      },
      { timeout: 5000 }
    );

    // Finish stream
    await finishStream();

    // Verify final state
    await waitFor(
      () => {
        const streaming = get(isStreaming);
        if (streaming) {
          throw new Error('Expected streaming to be false');
        }

        const tempId = get(streamingMessageId);
        if (tempId !== null) {
          throw new Error('Expected streaming message ID to be cleared');
        }

        // Verify no animated dots
        const dots = document.querySelector('.streaming-dots');
        if (dots) {
          throw new Error('Expected streaming indicator to be hidden after finish');
        }
      },
      { timeout: 5000 }
    );

    // Verify message properly rendered with final content
    await waitFor(
      () => {
        const currentMessages = get(messages);
        const assistantMsg = currentMessages.find(m => m.role === 'assistant');
        if (!assistantMsg || assistantMsg.content !== "You're absolutely right! I should do it like") {
          throw new Error(`Expected final message content, got: ${assistantMsg?.content}`);
        }
        // Verify message has a real numeric ID (not temp string)
        if (typeof assistantMsg.id !== 'number') {
          throw new Error('Expected message to have real numeric ID after stream finished');
        }
      },
      { timeout: 5000 }
    );
  });
});
