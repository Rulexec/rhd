import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { spawn, ChildProcess } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { get } from 'svelte/store';
import { waitForWebSocket, setControlPort, setWsPort, configureMock } from '../testUtils';
import { setWsPort as setWsWsPort, connectWebSocket } from '../../lib/ws';
import { wsConnected } from '../../lib/stores';
import { messages, isStreaming } from '../../lib/chatStores';
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

describe('Chat message flow', () => {
  it('sends message and receives response without duplication', async () => {
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

    const newChatButton = screen.getByText('+ New Chat');
    await fireEvent.click(newChatButton);

    const titleInput = screen.getByPlaceholderText('Enter chat title') as HTMLInputElement;
    await fireEvent.input(titleInput, { target: { value: 'Test Chat' } });

    const createButton = screen.getByText('Create');
    await fireEvent.click(createButton);

    await waitFor(
      () => {
        const modelSelect = screen.getByLabelText('Model:') as HTMLSelectElement;
        if (!modelSelect) {
          throw new Error('Model select element should be present');
        }
      },
      { timeout: 5000 }
    );

    await waitFor(
      () => {
        const modelSelect = screen.getByLabelText('Model:') as HTMLSelectElement;
        if (modelSelect.options.length === 0) {
          throw new Error('Model options should be loaded');
        }
      },
      { timeout: 5000 }
    );

    await configureMock('Test response from AI');

    const textarea = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
    textarea.value = 'Hello, AI!';
    await fireEvent.input(textarea);

    const sendButton = screen.getByText('Send');
    await fireEvent.click(sendButton);

    await waitFor(
      () => {
        const currentMessages = get(messages);
        const userMessages = currentMessages.filter(m => m.role === 'user' && m.content === 'Hello, AI!');
        if (userMessages.length !== 1) {
          throw new Error(`Expected exactly 1 user message, got ${userMessages.length}`);
        }
      },
      { timeout: 10000 }
    );

    await waitFor(
      () => {
        const currentMessages = get(messages);
        const assistantMessages = currentMessages.filter(m => m.role === 'assistant' && m.content === 'Test response from AI');
        if (assistantMessages.length !== 1) {
          throw new Error(`Expected exactly 1 assistant message, got ${assistantMessages.length}`);
        }
      },
      { timeout: 10000 }
    );

    await waitFor(
      () => {
        const streaming = get(isStreaming);
        if (streaming) {
          throw new Error('Expected streaming to be false');
        }
      },
      { timeout: 5000 }
    );

    await waitFor(
      () => {
        const textareaAfter = screen.getByPlaceholderText('Type a message...') as HTMLTextAreaElement;
        if (textareaAfter.disabled) {
          throw new Error('Expected textarea to be enabled');
        }
      },
      { timeout: 5000 }
    );

    await waitFor(
      () => {
        const sendButtonAfter = screen.queryByText('Send');
        const abortButton = screen.queryByText('Abort');
        if (!sendButtonAfter) {
          throw new Error('Expected Send button to be visible');
        }
        if (abortButton) {
          throw new Error('Expected Abort button to be hidden');
        }
      },
      { timeout: 5000 }
    );
  });
});
