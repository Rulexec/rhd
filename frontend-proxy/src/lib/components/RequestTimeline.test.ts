import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import RequestTimeline from './RequestTimeline.svelte';
import RequestTimelineHarness from './RequestTimelineHarness.svelte';
import type { RequestSummary } from '../api/schemas.js';

const request = (id: number, overrides: Partial<RequestSummary> = {}): RequestSummary => ({
  id,
  chatId: 1,
  ts: '2026-09-27T10:00:00Z',
  method: 'POST',
  path: '/v1/chat/completions',
  model: 'z-ai/glm-5.3',
  stream: false,
  status: 200,
  durationMs: null,
  error: null,
  hasAssembled: false,
  ...overrides
});

afterEach(() => {
  cleanup();
});

/** The timeline starts collapsed; row-level tests expand it via the header. */
async function expand(getByTestId: (id: string) => HTMLElement): Promise<void> {
  await fireEvent.click(getByTestId('requests-toggle'));
}

describe('RequestTimeline', () => {
  it('should be collapsed by default and toggle via the header', async () => {
    const requests = [request(1), request(2, { model: 'gpt-4o' })];

    const { getByTestId, queryAllByRole } = render(RequestTimeline, {
      props: { requests }
    });

    // Collapsed: header with count, no rows.
    const toggle = getByTestId('requests-toggle');
    expect(toggle.textContent).toContain('Requests (2)');
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(queryAllByRole('option')).toHaveLength(0);

    // Expand: rows appear.
    await expand(getByTestId);
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    expect(queryAllByRole('option')).toHaveLength(2);

    // Collapse again: rows disappear.
    await fireEvent.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect(queryAllByRole('option')).toHaveLength(0);
  });

  it('should show the selected request in the collapsed header', () => {
    const requests = [request(1), request(2)];

    const { getByTestId } = render(RequestTimeline, {
      props: { requests, selectedRequestId: 2 }
    });

    const toggle = getByTestId('requests-toggle');
    expect(toggle.textContent).toContain('Requests (2)');
    expect(toggle.textContent).toContain('#2 selected');
  });

  it('should render one row per request with model and count header once expanded', async () => {
    const requests = [request(1), request(2, { model: 'gpt-4o' })];

    const { getByTestId, getByText, getAllByRole } = render(RequestTimeline, {
      props: { requests }
    });

    await expand(getByTestId);

    expect(getByTestId('requests-toggle').textContent).toContain('Requests (2)');
    expect(getByText('z-ai/glm-5.3')).toBeTruthy();
    expect(getByText('gpt-4o')).toBeTruthy();
    expect(getAllByRole('option')).toHaveLength(2);
  });

  it('should render completed, in-flight, and errored statuses distinctly', async () => {
    const requests = [
      request(1, { status: 200, durationMs: 50 }),
      request(2, { status: null, error: null }),
      request(3, { status: null, error: 'upstream connect refused' })
    ];

    const { getByTestId, getByText, container } = render(RequestTimeline, {
      props: { requests }
    });

    await expand(getByTestId);

    const ok = getByText('200');
    expect(ok.className).toContain('st-ok');

    const pending = getByText('⏳');
    expect(pending.className).toContain('st-pending');

    const errored = getByText('ERR');
    expect(errored.className).toContain('st-error');

    // The errored row exposes the error text via its title tooltip.
    const errorRow = container.querySelector('li[title="upstream connect refused"]');
    expect(errorRow).toBeTruthy();
    expect(errorRow?.textContent).toContain('ERR');
  });

  it('should render the SSE badge only for streaming requests', async () => {
    const requests = [request(1, { stream: true }), request(2, { stream: false })];

    const { getByTestId, queryAllByText } = render(RequestTimeline, { props: { requests } });

    await expand(getByTestId);

    expect(queryAllByText('SSE')).toHaveLength(1);
  });

  it('should render durations with ms suffix and a dash when null', async () => {
    const requests = [request(1, { durationMs: 1234 }), request(2, { durationMs: null })];

    const { getByTestId, getByText } = render(RequestTimeline, { props: { requests } });

    await expand(getByTestId);

    expect(getByText('1234ms')).toBeTruthy();
    expect(getByText('—')).toBeTruthy();
  });

  it('should dispatch requestSelect on row click and mark the selected row', async () => {
    const requests = [request(1), request(2, { model: 'gpt-4o' })];
    const onRequestSelect = vi.fn();

    const { getByTestId, getByText, container } = render(RequestTimelineHarness, {
      props: { requests, selectedRequestId: 2, onRequestSelect }
    });

    await expand(getByTestId);

    const selected = container.querySelectorAll('[aria-selected="true"]');
    expect(selected).toHaveLength(1);
    expect(selected[0]?.textContent).toContain('gpt-4o');

    await fireEvent.click(getByText('z-ai/glm-5.3'));

    expect(onRequestSelect).toHaveBeenCalledWith({ requestId: 1 });
  });
});
