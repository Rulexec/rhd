import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/svelte';
import RawBodyView from './RawBodyView.svelte';

afterEach(() => {
  cleanup();
});

describe('RawBodyView', () => {
  it('should pretty-print valid JSON bodies and show the label', () => {
    const { container, getByText } = render(RawBodyView, {
      props: { body: '{"a":1}', label: 'Request Body' }
    });

    const pre = container.querySelector('.raw-body-pre');
    expect(pre).toBeTruthy();
    expect(pre?.textContent).toBe(JSON.stringify({ a: 1 }, null, 2));
    expect(getByText('Request Body')).toBeTruthy();
  });

  it('should render non-JSON (SSE) text verbatim', () => {
    const sse = 'data: {"delta":"hi"}\n\ndata: [DONE]\n';
    const { container } = render(RawBodyView, { props: { body: sse } });

    const pre = container.querySelector('.raw-body-pre');
    expect(pre?.textContent).toBe(sse);
  });

  it('should render the empty placeholder for a null body', () => {
    const { getByText, container } = render(RawBodyView, { props: { body: null } });

    expect(getByText('— nothing recorded —')).toBeTruthy();
    expect(container.querySelector('.raw-body-pre')).toBeNull();
  });
});
