import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { spawn, ChildProcess } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { get } from 'svelte/store';
import { waitForWebSocket, setControlPort, setWsPort } from '../testUtils';
import { setWsPort as setWsWsPort, connectWebSocket } from '../../lib/ws';
import { wsConnected } from '../../lib/stores';
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

describe('Chat UI', () => {
  it('creates new chat with model pre-selected', async () => {
    if (!window.prompt) {
      (window as any).prompt = () => null;
    }
    vi.spyOn(window, 'prompt').mockReturnValue('Test Chat');

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

    await waitFor(
      () => {
        const modelSelect = screen.getByLabelText('Model:') as HTMLSelectElement;
        const expectedValue = modelSelect.options[0].value;
        if (modelSelect.value !== expectedValue) {
          throw new Error(`First model should be auto-selected: expected "${expectedValue}", got "${modelSelect.value}"`);
        }
      },
      { timeout: 5000 }
    );
  });
});
