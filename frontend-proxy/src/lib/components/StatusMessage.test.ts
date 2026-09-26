import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/svelte';
import StatusMessage from './StatusMessage.svelte';

afterEach(() => {
  cleanup();
});

describe('StatusMessage', () => {
  it('should show loading text when loading', () => {
    const { getByTestId, container } = render(StatusMessage, {
      props: { loading: true, error: null, empty: false }
    });

    expect(getByTestId('status-loading')).toBeTruthy();
    expect(container.textContent).toContain('Loading...');
  });

  it('should use custom loadingText when provided', () => {
    const { container } = render(StatusMessage, {
      props: { loading: true, error: null, empty: false, loadingText: 'Loading chats...' }
    });

    expect(container.textContent).toContain('Loading chats...');
  });

  it('should show error text with status-error test id', () => {
    const { getByTestId } = render(StatusMessage, {
      props: { loading: false, error: 'boom', empty: false }
    });

    expect(getByTestId('status-error').textContent).toContain('boom');
  });

  it('should show empty text when empty and not loading or error', () => {
    const { getByTestId } = render(StatusMessage, {
      props: { loading: false, error: null, empty: true }
    });

    expect(getByTestId('status-empty').textContent).toContain('Nothing here yet');
  });

  it('should use custom emptyText when provided', () => {
    const { getByTestId } = render(StatusMessage, {
      props: { loading: false, error: null, empty: true, emptyText: 'No logged chats yet' }
    });

    expect(getByTestId('status-empty').textContent).toContain('No logged chats yet');
  });

  it('should render nothing when no flag is set', () => {
    const { container } = render(StatusMessage, {
      props: { loading: false, error: null, empty: false }
    });

    expect(container.querySelector('[data-testid^="status-"]')).toBeNull();
    expect(container.textContent).toBe('');
  });
});
