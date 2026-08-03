import { render, screen, waitFor, fireEvent, within } from '@testing-library/svelte';
import { pausedScenarios, finishedScenarios, wsConnected } from '@/lib/stores';
import ScenariosTab from '@/components/ScenariosTab.svelte';
import { retryScenario } from '@/lib/ws';

vi.mock('@/lib/ws', () => ({
  connectWebSocket: vi.fn(),
  subscribe: vi.fn().mockResolvedValue({ success: true, data: { activeExecutions: [], pausedExecutions: [] } }),
  getFinishedScenarios: vi.fn().mockResolvedValue({ success: true, data: [] }),
  retryScenario: vi.fn(),
  abortScenario: vi.fn(),
  abortScenarioWithError: vi.fn(),
  setWsPort: vi.fn(),
  initWebSocket: vi.fn(),
  generateRequestId: vi.fn().mockReturnValue('req-1'),
  sendRequest: vi.fn(),
}));

vi.mock('@/lib/notifications', () => ({
  showNotification: vi.fn(),
}));

describe('PausedScenario retry', () => {
  let retryResolve: (value: { type: 'response'; id: string; success: boolean; data?: unknown }) => void;

  beforeEach(() => {
    vi.clearAllMocks();
    pausedScenarios.removeScenario('exec-1');
    finishedScenarios.setAll([]);
    wsConnected.set(true);
  });

  it('shows loading indicator on retry click and moves to finished list', async () => {
    const retryPromise = new Promise<{ type: 'response'; id: string; success: boolean; data?: unknown }>((resolve) => {
      retryResolve = resolve;
    });
    vi.mocked(retryScenario).mockReturnValue(retryPromise);

    pausedScenarios.addScenario({
      executionId: 'exec-1',
      scenarioName: 'test_retry_scenario',
      error: 'Test error',
      stepName: 'step1',
      availableModels: ['model1'],
      selectedModel: 'model1',
    });

    render(ScenariosTab);

    await waitFor(() => {
      expect(screen.getByText('test_retry_scenario')).toBeInTheDocument();
    });

    expect(screen.getByText('Paused')).toBeInTheDocument();

    const retryButton = screen.getByText('Retry');
    fireEvent.click(retryButton);

    await waitFor(() => {
      const retryingButton = screen.getByText('Retrying...');
      expect(retryingButton).toBeInTheDocument();
      expect(retryingButton).toBeDisabled();
    });

    retryResolve({ type: 'response', id: 'req-1', success: true });

    await new Promise((resolve) => setTimeout(resolve, 0));

    pausedScenarios.removeScenario('exec-1');
    finishedScenarios.prepend({
      id: 1,
      scenario: 'test_retry_scenario',
      status: 'success',
      finished: new Date().toISOString(),
      durationMs: 1000,
    });

    await waitFor(() => {
      expect(screen.getByText('Finished')).toBeInTheDocument();
    });

    const finishedList = screen.getByTestId('finished-list');
    expect(within(finishedList).getByText('test_retry_scenario')).toBeInTheDocument();

    expect(screen.queryByText('Paused')).not.toBeInTheDocument();
  });

  it('hides paused scenario after retry succeeds and execution continues to next step', async () => {
    const retryPromise = new Promise<{ type: 'response'; id: string; success: boolean; data?: unknown }>((resolve) => {
      retryResolve = resolve;
    });
    vi.mocked(retryScenario).mockReturnValue(retryPromise);

    pausedScenarios.addScenario({
      executionId: 'exec-2',
      scenarioName: 'test_continue_after_retry',
      error: 'AI request failed',
      stepName: 'aiChat',
      availableModels: ['model1', 'model2'],
      selectedModel: 'model1',
    });

    render(ScenariosTab);

    await waitFor(() => {
      expect(screen.getByText('test_continue_after_retry')).toBeInTheDocument();
    });

    expect(screen.getByText('Paused')).toBeInTheDocument();

    const retryButton = screen.getByText('Retry');
    fireEvent.click(retryButton);

    await waitFor(() => {
      expect(screen.getByText('Retrying...')).toBeInTheDocument();
    });

    retryResolve({ type: 'response', id: 'req-1', success: true });

    await new Promise((resolve) => setTimeout(resolve, 0));

    pausedScenarios.removeScenario('exec-2');

    await waitFor(() => {
      expect(screen.queryByText('Paused')).not.toBeInTheDocument();
      expect(screen.queryByText('test_continue_after_retry')).not.toBeInTheDocument();
    });
  });
});
