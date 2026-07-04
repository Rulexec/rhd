import { render, screen, waitFor } from '@testing-library/svelte';
import { spawn, ChildProcess } from 'child_process';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { get } from 'svelte/store';
import {
  waitForWebSocket,
  setControlPort,
  setWsPort,
  injectFinishedScenario,
  testScenarioStarted,
  testScenarioFinished,
} from '../testUtils';
import { setWsPort as setWsWsPort, connectWebSocket } from '../../lib/ws';
import { wsConnected, activeScenariosList, finishedScenarios } from '../../lib/stores';
import ScenariosTab from '../../components/ScenariosTab.svelte';

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

describe('Scenarios Tab', () => {
  it('displays pre-created finished scenarios', async () => {
    await waitFor(
      () => {
        const wsState = get(wsConnected);
        if (!wsState) {
          throw new Error('WebSocket not connected');
        }
      },
      { timeout: 5000 }
    );

    // Inject 2 finished scenarios
    const now = new Date().toISOString();
    await injectFinishedScenario({
      id: 1,
      scenario: 'test_scenario_1',
      status: 'success',
      started: now,
      finished: now,
      durationMs: 1000,
    });
    await injectFinishedScenario({
      id: 2,
      scenario: 'test_scenario_2',
      status: 'success',
      started: now,
      finished: now,
      durationMs: 2000,
    });

    render(ScenariosTab);

    // Wait for finished scenarios to appear
    await waitFor(
      () => {
        const finished = get(finishedScenarios);
        if (finished.length < 2) {
          throw new Error(`Expected 2 finished scenarios, got ${finished.length}`);
        }
      },
      { timeout: 5000 }
    );

    // Verify both scenarios are shown
    expect(screen.getByText('test_scenario_1')).toBeInTheDocument();
    expect(screen.getByText('test_scenario_2')).toBeInTheDocument();
  });

  it('shows active scenario when started and moves to finished when complete', async () => {
    // Render the ScenariosTab component
    render(ScenariosTab);

    // Trigger scenario started via IPC
    await testScenarioStarted(100, 'test_active_scenario');

    // Wait for scenario to appear in active list
    await waitFor(
      () => {
        const active = get(activeScenariosList);
        if (active.length === 0) {
          throw new Error('Expected active scenario to appear');
        }
      },
      { timeout: 5000 }
    );

    // Verify active scenario is shown
    expect(screen.getByText('test_active_scenario')).toBeInTheDocument();

    // Trigger scenario finished via IPC
    await testScenarioFinished(100, 'test_active_scenario');

    // Wait for finished scenarios to update
    await waitFor(
      () => {
        const finished = get(finishedScenarios);
        if (finished.length < 3) {
          throw new Error(`Expected 3 finished scenarios, got ${finished.length}`);
        }
      },
      { timeout: 5000 }
    );

    // Verify 3 finished scenarios (2 pre-created + 1 new)
    const finished = get(finishedScenarios);
    expect(finished.length).toBe(3);
  });
});
