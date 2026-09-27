import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/svelte';
import ConversationView from './ConversationView.svelte';
import type { ConversationTurn, MessageTurn } from '../api/schemas.js';

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

  it('should show a source badge on message turns', () => {
    const conversation = [
      messageTurn({ seq: 0, source: 'history', content: 'hi' }),
      messageTurn({ seq: 1, role: 'assistant', source: 'response', content: 'hello' })
    ];

    const { getAllByTestId } = render(ConversationView, { props: { conversation } });

    const badges = getAllByTestId('source-badge');
    expect(badges.map((badge) => badge.textContent)).toEqual(['history', 'response']);
  });

  it('should render a pending tail turn with its request id', () => {
    const conversation = [pendingTurn(7)];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    const turn = getByTestId('turn-pending');
    expect(turn.textContent).toContain('⏳');
    expect(turn.textContent).toContain('7');
  });

  it('should render an error tail turn with the error text', () => {
    const conversation = [errorTurn(7, 'upstream connect refused')];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    const turn = getByTestId('turn-error');
    expect(turn.textContent).toContain('✗');
    expect(turn.textContent).toContain('upstream connect refused');
  });
});
