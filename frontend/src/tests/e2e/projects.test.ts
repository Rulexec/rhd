import { render, screen, waitFor } from '@testing-library/svelte';
import { spawn, ChildProcess } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { get } from 'svelte/store';
import { waitForWebSocket, setControlPort, setWsPort } from '../testUtils';
import { setWsPort as setWsWsPort, connectWebSocket } from '../../lib/ws';
import { wsConnected } from '../../lib/stores';
import ProjectsPanel from '../../components/ProjectsPanel.svelte';

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

describe('Projects UI', () => {
  it('renders projects panel', async () => {
    await waitFor(
      () => {
        const wsState = get(wsConnected);
        if (!wsState) {
          throw new Error('WebSocket not connected');
        }
      },
      { timeout: 5000 }
    );

    render(ProjectsPanel);

    await waitFor(
      () => {
        const heading = screen.getByRole('heading', { name: /projects/i });
        if (!heading) {
          throw new Error('Projects heading should be present');
        }
      },
      { timeout: 5000 }
    );
  });

  it('displays available projects', async () => {
    render(ProjectsPanel);

    await waitFor(
      () => {
        // Check if projects list is rendered (may be empty or have items)
        const heading = screen.queryByRole('heading', { name: /projects/i });
        if (!heading) {
          throw new Error('Projects heading should be present');
        }
      },
      { timeout: 5000 }
    );
  });
});
