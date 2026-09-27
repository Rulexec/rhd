import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import ConversationView from './ConversationView.svelte';
import ConversationViewHarness from './ConversationViewHarness.svelte';
import type { ConversationTurn, MessageTurn, RequestDetail } from '../api/schemas.js';

const messageTurn = (overrides: Partial<MessageTurn> = {}): MessageTurn => ({
  kind: 'message',
  seq: 0,
  role: 'user',
  source: 'history',
  content: null,
  toolCalls: null,
  toolCallId: null,
  name: null,
  requestId: 1,
  ...overrides
});

const pendingTurn = (requestId = 7): ConversationTurn => ({ kind: 'pending', requestId });

const errorTurn = (requestId = 7, error = 'boom'): ConversationTurn => ({
  kind: 'error',
  requestId,
  error
});

const requestDetail = (id = 5): RequestDetail => ({
  summary: {
    id,
    chatId: 1,
    ts: '2026-09-27T10:00:00Z',
    method: 'POST',
    path: '/v1/chat/completions',
    model: 'test-model',
    stream: false,
    status: 200,
    durationMs: 42,
    error: null,
    hasAssembled: true
  },
  requestBody: '{"model":"test-model"}',
  responseBody: null,
  responseAssembled: null
});

afterEach(() => {
  cleanup();
});

describe('ConversationView', () => {
  it('should render message turns with role labels and string contents', () => {
    const conversation = [
      messageTurn({ seq: 0, role: 'system', content: 'You are helpful.' }),
      messageTurn({ seq: 1, role: 'user', content: 'Hello there' })
    ];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    const system = getByTestId('turn-system');
    expect(system.textContent).toContain('system');
    expect(system.textContent).toContain('You are helpful.');
    const user = getByTestId('turn-user');
    expect(user.textContent).toContain('user');
    expect(user.textContent).toContain('Hello there');
  });

  it('should render non-string content (array of parts) as JSON without crashing', () => {
    const parts = [{ type: 'text', text: 'multi-part hello' }];
    const conversation = [messageTurn({ content: parts })];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    const body = getByTestId('turn-user').textContent ?? '';
    expect(body).toContain('"type": "text"');
    expect(body).toContain('multi-part hello');
  });

  it('should show a placeholder for null content without tool calls', () => {
    const conversation = [messageTurn({ role: 'assistant', content: null })];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    expect(getByTestId('turn-assistant').textContent).toContain('—');
  });

  it('should markdown-render every assistant row with string content', () => {
    const conversation = [
      messageTurn({
        seq: 2,
        role: 'assistant',
        source: 'response',
        content: '**earlier** reply'
      }),
      messageTurn({
        seq: 4,
        role: 'assistant',
        source: 'response',
        content: '**latest** reply'
      })
    ];

    const { getAllByTestId } = render(ConversationView, { props: { conversation } });

    const turns = getAllByTestId('turn-assistant');
    expect(turns).toHaveLength(2);
    expect(turns[0]?.querySelector('strong')?.textContent).toBe('earlier');
    expect(turns[1]?.querySelector('strong')?.textContent).toBe('latest');
  });

  it('should render tool calls with function name, arguments and call id', () => {
    const conversation = [
      messageTurn({
        seq: 2,
        role: 'assistant',
        source: 'response',
        content: null,
        toolCalls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'get_weather', arguments: '{"city":"Oslo"}' }
          }
        ]
      })
    ];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    const calls = getByTestId('tool-calls');
    expect(calls.textContent).toContain('get_weather');
    expect(calls.textContent).toContain('call_1');
    // Arguments are pretty-printed.
    expect(calls.textContent).toContain('"city": "Oslo"');
  });

  it('should render tool result turns with name, pretty content and tool_call_id', () => {
    const conversation = [
      messageTurn({
        seq: 3,
        role: 'tool',
        content: '{"temp": 20}',
        toolCallId: 'call_1',
        name: 'get_weather'
      })
    ];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    const turn = getByTestId('turn-tool');
    expect(turn.textContent).toContain('tool: get_weather');
    expect(turn.textContent).toContain('tool_call_id: call_1');
    // String content holding JSON is pretty-printed.
    expect(turn.textContent).toContain('"temp": 20');
  });

  it('should not render source badges on message turns', () => {
    const conversation = [
      messageTurn({ seq: 0, source: 'history', content: 'hi' }),
      messageTurn({ seq: 1, role: 'assistant', source: 'response', content: 'hello' })
    ];

    const { queryAllByTestId } = render(ConversationView, { props: { conversation } });

    expect(queryAllByTestId('source-badge')).toHaveLength(0);
  });

  it('should anchor message turns with their seq for scroll targeting', () => {
    const conversation = [
      messageTurn({ seq: 0, role: 'system', content: 'sys' }),
      messageTurn({ seq: 1, role: 'user', content: 'hi' })
    ];

    const { container } = render(ConversationView, { props: { conversation } });

    expect(container.querySelector('[data-turn-seq="1"]')).toBeTruthy();
    expect(container.querySelector('[data-turn-seq="42"]')).toBeNull();
  });

  it('should render a raw-request button on every turn and dispatch requestSelect on click', async () => {
    const conversation = [
      messageTurn({ seq: 0, role: 'system', content: 'sys', requestId: 5 }),
      messageTurn({ seq: 1, role: 'user', content: 'hi', requestId: 5 }),
      pendingTurn(7),
      errorTurn(8)
    ];
    const onRequestSelect = vi.fn();

    const { getAllByTestId } = render(ConversationViewHarness, {
      props: { conversation, onRequestSelect }
    });

    const buttons = getAllByTestId('turn-request-button');
    expect(buttons).toHaveLength(4);
    expect(buttons[0]?.textContent).toContain('raw #5');
    expect(buttons[2]?.textContent).toContain('raw #7');
    expect(buttons[3]?.textContent).toContain('raw #8');

    // Message turns anchor by seq; the pending/error tails anchor at 'tail'.
    await fireEvent.click(buttons[1]!);
    expect(onRequestSelect).toHaveBeenCalledWith({ requestId: 5, anchor: 1 });

    await fireEvent.click(buttons[2]!);
    expect(onRequestSelect).toHaveBeenCalledWith({ requestId: 7, anchor: 'tail' });
  });

  it('should render the drill-down inline right after the anchored message turn', () => {
    const conversation = [
      messageTurn({ seq: 0, role: 'system', content: 'sys' }),
      messageTurn({ seq: 1, role: 'user', content: 'hi' }),
      messageTurn({ seq: 2, role: 'assistant', source: 'response', content: 'yo' })
    ];

    const { container, getByTestId } = render(ConversationView, {
      props: { conversation, requestDetail: requestDetail(), openAnchor: 1 }
    });

    const detailEl = getByTestId('request-detail');
    const children = Array.from(container.querySelector('.conversation')!.children);
    expect(children.indexOf(container.querySelector('[data-turn-seq="0"]')!)).toBe(0);
    expect(children.indexOf(container.querySelector('[data-turn-seq="1"]')!)).toBe(1);
    // Directly after the anchored turn, before the following turns.
    expect(children.indexOf(detailEl)).toBe(2);
    expect(children.indexOf(container.querySelector('[data-turn-seq="2"]')!)).toBe(3);
  });

  it('should not render the inline drill-down for a mismatched anchor, a null anchor, or no detail', () => {
    const conversation = [messageTurn({ seq: 1, role: 'user', content: 'hi' })];

    // Anchor matches no turn's seq.
    const mismatched = render(ConversationView, {
      props: { conversation, requestDetail: requestDetail(), openAnchor: 42 }
    });
    expect(mismatched.queryByTestId('request-detail')).toBeNull();

    // Null anchor means the bottom placement (below the timeline).
    const nullAnchor = render(ConversationView, {
      props: { conversation, requestDetail: requestDetail(), openAnchor: null }
    });
    expect(nullAnchor.queryByTestId('request-detail')).toBeNull();

    // No detail loaded yet — nothing to render even with a matching anchor.
    const noDetail = render(ConversationView, {
      props: { conversation, requestDetail: null, openAnchor: 1 }
    });
    expect(noDetail.queryByTestId('request-detail')).toBeNull();
  });

  it("should render the drill-down after the tail for the 'tail' anchor", () => {
    const pendingConversation: ConversationTurn[] = [
      messageTurn({ seq: 0, role: 'user', content: 'hi' }),
      pendingTurn(7)
    ];

    const pending = render(ConversationView, {
      props: { conversation: pendingConversation, requestDetail: requestDetail(), openAnchor: 'tail' }
    });
    const pendingDetail = pending.container.querySelector('[data-testid="request-detail"]')!;
    const pendingChildren = Array.from(pending.container.querySelector('.conversation')!.children);
    expect(pendingChildren.indexOf(pendingDetail)).toBe(2);
    expect(pendingChildren.at(-1)).toBe(pendingDetail);

    const errorConversation: ConversationTurn[] = [errorTurn(7, 'upstream refused')];
    const errored = render(ConversationView, {
      props: { conversation: errorConversation, requestDetail: requestDetail(), openAnchor: 'tail' }
    });
    const errorDetail = errored.container.querySelector('[data-testid="request-detail"]')!;
    const errorChildren = Array.from(errored.container.querySelector('.conversation')!.children);
    expect(errorChildren.at(-1)).toBe(errorDetail);
  });

  it('should invoke onCloseRequest from the inline drill-down close button', async () => {
    const conversation = [messageTurn({ seq: 1, role: 'user', content: 'hi' })];
    const onCloseRequest = vi.fn();

    const { getByTestId } = render(ConversationViewHarness, {
      props: { conversation, requestDetail: requestDetail(), openAnchor: 1, onCloseRequest }
    });

    await fireEvent.click(getByTestId('request-detail-close'));

    expect(onCloseRequest).toHaveBeenCalledTimes(1);
  });

  it('should render a pending tail turn with a raw-request button', () => {
    const conversation = [pendingTurn(7)];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    const turn = getByTestId('turn-pending');
    expect(turn.textContent).toContain('⏳');
    expect(getByTestId('turn-request-button').textContent).toContain('raw #7');
  });

  it('should render an error tail turn with the error text', () => {
    const conversation = [errorTurn(7, 'upstream connect refused')];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    const turn = getByTestId('turn-error');
    expect(turn.textContent).toContain('✗');
    expect(turn.textContent).toContain('upstream connect refused');
  });
});
