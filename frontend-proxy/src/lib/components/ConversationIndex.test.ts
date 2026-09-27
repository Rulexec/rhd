import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import ConversationIndex from './ConversationIndex.svelte';
import ConversationIndexHarness from './ConversationIndexHarness.svelte';
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

const conversationWithEverything: ConversationTurn[] = [
  messageTurn({ seq: 0, role: 'system', content: 'You are helpful.' }),
  messageTurn({ seq: 1, role: 'user', content: 'Fix the login bug in auth.ts' }),
  messageTurn({
    seq: 2,
    role: 'assistant',
    source: 'response',
    content: null,
    toolCalls: [{ id: 'call_1', type: 'function', function: { name: 'read_file' } }]
  }),
  messageTurn({
    seq: 3,
    role: 'tool',
    content: '{}',
    toolCallId: 'call_1',
    name: 'read_file'
  }),
  messageTurn({ seq: 4, role: 'assistant', source: 'response', content: 'Fixed it' }),
  { kind: 'pending', requestId: 9 }
];

afterEach(() => {
  cleanup();
});

describe('ConversationIndex', () => {
  it('should index user messages and tool-less assistant responses only', () => {
    const { getAllByTestId } = render(ConversationIndex, {
      props: { conversation: conversationWithEverything }
    });

    const entries = getAllByTestId('index-entry');
    expect(entries).toHaveLength(2);
    expect(entries[0]?.textContent).toContain('Fix the login bug in auth.ts');
    expect(entries[1]?.textContent).toContain('Fixed it');
  });

  it('should show the entry count in the header', () => {
    const { getByText } = render(ConversationIndex, {
      props: { conversation: conversationWithEverything }
    });

    expect(getByText('Index (2)')).toBeTruthy();
  });

  it('should build previews from string content and parts-array content', () => {
    const conversation: ConversationTurn[] = [
      messageTurn({
        seq: 1,
        role: 'user',
        content: [{ type: 'text', text: 'hello ' }, { type: 'text', text: 'world' }]
      }),
      messageTurn({ seq: 2, role: 'assistant', source: 'response', content: null })
    ];

    const { getAllByTestId } = render(ConversationIndex, { props: { conversation } });

    const entries = getAllByTestId('index-entry');
    expect(entries[0]?.textContent).toContain('hello world');
    expect(entries[1]?.textContent).toContain('—');
  });

  it('should dispatch entrySelect with the turn seq on entry click', async () => {
    const onEntrySelect = vi.fn();

    const { getAllByTestId } = render(ConversationIndexHarness, {
      props: { conversation: conversationWithEverything, onEntrySelect }
    });

    await fireEvent.click(getAllByTestId('index-entry')[1]!);

    expect(onEntrySelect).toHaveBeenCalledWith({ seq: 4 });
  });

  it('should show the empty state when no entries match', () => {
    const conversation: ConversationTurn[] = [
      messageTurn({ seq: 0, role: 'system', content: 'sys' }),
      messageTurn({ seq: 1, role: 'tool', content: '{}', toolCallId: 'c1', name: 'f' })
    ];

    const { getByText, queryAllByTestId } = render(ConversationIndex, {
      props: { conversation }
    });

    expect(getByText('No user or assistant messages')).toBeTruthy();
    expect(queryAllByTestId('index-entry')).toHaveLength(0);
  });
});
