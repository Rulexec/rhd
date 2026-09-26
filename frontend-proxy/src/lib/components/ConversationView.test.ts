import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/svelte';
import ConversationView from './ConversationView.svelte';
import type { AssistantTurn, ConversationTurn } from '../api/schemas.js';

const messageTurn = (role: string, content: unknown): ConversationTurn => ({
  kind: 'message',
  role,
  content
});

const assistantTurn = (overrides: Partial<AssistantTurn> = {}): ConversationTurn => ({
  kind: 'assistant',
  requestId: 7,
  content: null,
  error: null,
  pending: false,
  ...overrides
});

afterEach(() => {
  cleanup();
});

describe('ConversationView', () => {
  it('should render message turns with role labels and string contents', () => {
    const conversation = [
      messageTurn('system', 'You are helpful.'),
      messageTurn('user', 'Hello there')
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
    const conversation = [messageTurn('user', parts)];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    const body = getByTestId('turn-user').textContent ?? '';
    expect(body).toContain('"type": "text"');
    expect(body).toContain('multi-part hello');
  });

  it('should markdown-render a completed assistant turn with string content', () => {
    const conversation = [
      assistantTurn({ content: JSON.stringify({ role: 'assistant', content: '**bold** reply' }) })
    ];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    const turn = getByTestId('turn-assistant');
    expect(turn.querySelector('strong')?.textContent).toBe('bold');
    expect(turn.textContent).toContain('reply');
  });

  it('should fall back to an escaped pre block for tool-call-only assembled JSON', () => {
    const assembled = JSON.stringify({
      role: 'assistant',
      content: null,
      tool_calls: [{ id: 'call_1', function: { name: 'lookup' } }]
    });
    const conversation = [assistantTurn({ content: assembled })];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    const turn = getByTestId('turn-assistant');
    const pre = turn.querySelector('pre');
    expect(pre).toBeTruthy();
    expect(pre?.textContent).toContain('tool_calls');
    expect(pre?.textContent).toContain('lookup');
  });

  it('should render a pending assistant turn with its request id', () => {
    const conversation = [assistantTurn({ pending: true })];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    const turn = getByTestId('turn-assistant-pending');
    expect(turn.textContent).toContain('⏳');
    expect(turn.textContent).toContain('7');
  });

  it('should render an errored assistant turn with the error text', () => {
    const conversation = [assistantTurn({ error: 'upstream connect refused' })];

    const { getByTestId } = render(ConversationView, { props: { conversation } });

    const turn = getByTestId('turn-assistant-error');
    expect(turn.textContent).toContain('✗');
    expect(turn.textContent).toContain('upstream connect refused');
  });
});
