import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import RequestDetailView from './RequestDetailView.svelte';
import type { RequestDetail } from '../api/schemas.js';

const detail: RequestDetail = {
  summary: {
    id: 42,
    chatId: 1,
    ts: '2026-09-27T10:00:00Z',
    method: 'POST',
    path: '/v1/chat/completions',
    model: 'z-ai/glm-5.3',
    stream: true,
    status: 200,
    durationMs: 1234,
    error: null,
    hasAssembled: true
  },
  requestBody: '{"model":"z-ai/glm-5.3","messages":[]}',
  responseBody: 'data: {"delta":"hi"}\n\ndata: [DONE]\n',
  responseAssembled: '{"role":"assistant","content":"hello"}'
};

afterEach(() => {
  cleanup();
});

describe('RequestDetailView', () => {
  it('should render request metadata (id, method, path, status, duration)', () => {
    const { getByTestId } = render(RequestDetailView, {
      props: { requestDetail: detail, onClose: vi.fn() }
    });

    const text = getByTestId('request-detail').textContent ?? '';
    expect(text).toContain('Request #42');
    expect(text).toContain('SSE');
    expect(text).toContain('POST /v1/chat/completions');
    expect(text).toContain('200');
    expect(text).toContain('1234ms');
  });

  it('should show the pretty-printed request body on the default tab', () => {
    const { container, getByRole } = render(RequestDetailView, {
      props: { requestDetail: detail, onClose: vi.fn() }
    });

    expect(getByRole('tab', { selected: true }).textContent).toBe('Raw Request');
    const pre = container.querySelector('.raw-body-pre');
    expect(pre?.textContent).toBe(
      JSON.stringify({ model: 'z-ai/glm-5.3', messages: [] }, null, 2)
    );
  });

  it('should switch to the response body and assembled reply tabs', async () => {
    const { container, getByText } = render(RequestDetailView, {
      props: { requestDetail: detail, onClose: vi.fn() }
    });

    await fireEvent.click(getByText('Raw Response'));
    expect(container.querySelector('.raw-body-pre')?.textContent).toBe(
      'data: {"delta":"hi"}\n\ndata: [DONE]\n'
    );

    await fireEvent.click(getByText('Assembled Reply'));
    expect(container.querySelector('.raw-body-pre')?.textContent).toBe(
      JSON.stringify({ role: 'assistant', content: 'hello' }, null, 2)
    );
  });

  it('should show the empty placeholder when responseBody is null (in flight)', async () => {
    const inFlight: RequestDetail = {
      ...detail,
      responseBody: null,
      summary: { ...detail.summary, status: null, durationMs: null, stream: false }
    };

    const { getByText } = render(RequestDetailView, {
      props: { requestDetail: inFlight, onClose: vi.fn() }
    });

    await fireEvent.click(getByText('Raw Response'));

    expect(getByText('— nothing recorded —')).toBeTruthy();
  });

  it('should call onClose when the close button is clicked', async () => {
    const onClose = vi.fn();

    const { getByTestId } = render(RequestDetailView, {
      props: { requestDetail: detail, onClose }
    });

    await fireEvent.click(getByTestId('request-detail-close'));

    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
