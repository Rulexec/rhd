import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import RefreshButtonHarness from './RefreshButtonHarness.svelte';
import { ProxyLogsStore } from '../../stores/ProxyLogsStore.js';
import { defaultProxyLogsApi } from '../api/ProxyLogsApi.js';

afterEach(() => {
  cleanup();
});

describe('RefreshButton', () => {
  it('should render enabled with default label when idle', () => {
    const mockStore = {
      refreshing: false,
      refresh: vi.fn()
    };
    const store = mockStore as unknown as ProxyLogsStore;

    const { getByTestId } = render(RefreshButtonHarness, { props: { store } });

    const button = getByTestId('refresh-button') as HTMLButtonElement;
    expect(button.disabled).toBe(false);
    expect(button.textContent).toBe('↻ Refresh');
  });

  it('should call store.refresh on click', async () => {
    const store = new ProxyLogsStore({ api: defaultProxyLogsApi });
    const refreshMock = vi.fn().mockResolvedValue(undefined);
    store.refresh = refreshMock as unknown as ProxyLogsStore['refresh'];

    const { getByTestId } = render(RefreshButtonHarness, { props: { store } });

    await fireEvent.click(getByTestId('refresh-button'));

    expect(refreshMock).toHaveBeenCalledTimes(1);
  });

  it('should be disabled and show progress while refreshing', () => {
    const mockStore = {
      refreshing: true,
      refresh: vi.fn()
    };
    const store = mockStore as unknown as ProxyLogsStore;

    const { getByTestId } = render(RefreshButtonHarness, { props: { store } });

    const button = getByTestId('refresh-button') as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    expect(button.textContent).toBe('Refreshing…');
  });
});
