import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import TabViewHarness from './TabViewHarness.svelte';

afterEach(() => {
  cleanup();
});

describe('TabView MCPs tab', () => {
  it('hides the MCPs button when showMcps is false', () => {
    const { queryByText } = render(TabViewHarness, { props: { showMcps: false } });

    expect(queryByText('Chats')).toBeTruthy();
    expect(queryByText('Plugins')).toBeTruthy();
    expect(queryByText('MCPs')).toBeNull();
  });

  it('shows the MCPs button and dispatches tabChange when clicked', async () => {
    const onTabChange = vi.fn();
    const { getByRole } = render(TabViewHarness, { props: { showMcps: true, onTabChange } });

    const mcpsButton = getByRole('tab', { name: 'MCPs' });
    expect(mcpsButton.getAttribute('aria-selected')).toBe('false');

    await fireEvent.click(mcpsButton);

    expect(onTabChange).toHaveBeenCalledWith({ tab: 'mcps' });
    expect(mcpsButton.getAttribute('aria-selected')).toBe('true');
  });
});
