import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import Message from './Message.svelte';
import type { Message as MessageType, ToolCall } from '../api/schemas.js';

afterEach(cleanup);

function choiceToolCall(overrides: Partial<ToolCall> = {}): ToolCall {
  return {
    id: 'call_1',
    type: 'function',
    function: {
      name: 'rhd_choice',
      arguments: JSON.stringify({ question: 'Which plan?', options: ['Alpha', 'Beta'] })
    },
    tags: [],
    ...overrides
  };
}

function assistantMessage(overrides: Partial<MessageType> = {}): MessageType {
  return {
    id: 1,
    chatId: 1,
    role: 'assistant',
    content: '',
    createdAt: '2026-09-10T00:00:00Z',
    tags: [],
    isFinished: true,
    isStreaming: false,
    toolCalls: [],
    ...overrides
  };
}

describe('Message rhd_choice rendering', () => {
  it('renders an interactive choice prompt and wires the responder callback', async () => {
    const onChoiceRespond = vi.fn();
    const message = assistantMessage({ toolCalls: [choiceToolCall()] });

    const { container, getByText } = render(Message, {
      props: { message, onChoiceRespond }
    });

    expect(container.querySelector('.choice-prompt')).toBeTruthy();
    expect(getByText('Which plan?')).toBeTruthy();

    await fireEvent.click(getByText('Alpha'));
    expect(onChoiceRespond).toHaveBeenCalledWith('call_1', 'Alpha');
  });

  it('shows the resolved state from toolResults', () => {
    const message = assistantMessage({ toolCalls: [choiceToolCall()] });
    const toolResults = new Map([['call_1', 'Alpha']]);

    const { container, getByText } = render(Message, {
      props: { message, toolResults, onChoiceRespond: vi.fn() }
    });

    expect(getByText('Answered:')).toBeTruthy();
    expect(getByText('Alpha')).toBeTruthy();
    expect(container.querySelector('.choice-option')).toBeNull();
  });

  it('falls back to the generic tool-call view for malformed choice args', () => {
    const message = assistantMessage({
      toolCalls: [choiceToolCall({ function: { name: 'rhd_choice', arguments: '{"question":' } })]
    });

    const { container } = render(Message, {
      props: { message, onChoiceRespond: vi.fn() }
    });

    expect(container.querySelector('.tool-call')).toBeTruthy();
    expect(container.querySelector('.choice-prompt')).toBeNull();
  });

  it('does not render the interactive prompt while streaming', () => {
    const message = assistantMessage({ isStreaming: true });
    const streamContent = {
      reasoningContent: '',
      content: '',
      toolCalls: [
        {
          id: 'call_1',
          name: 'rhd_choice',
          arguments: JSON.stringify({ question: 'Which plan?', options: ['Alpha'] })
        }
      ],
      isFinished: false
    };

    const { container } = render(Message, {
      props: { message, streamContent, onChoiceRespond: vi.fn() }
    });

    expect(container.querySelector('.choice-prompt')).toBeNull();
    expect(container.querySelector('.tool-call')).toBeTruthy();
  });

  it('renders non-choice tool calls generically', () => {
    const message = assistantMessage({
      toolCalls: [
        choiceToolCall({
          id: 'call_2',
          function: { name: 'get_weather', arguments: '{"city":"Minsk"}' }
        })
      ]
    });

    const { container, getByText } = render(Message, {
      props: { message, onChoiceRespond: vi.fn() }
    });

    expect(container.querySelector('.tool-call')).toBeTruthy();
    expect(container.querySelector('.choice-prompt')).toBeNull();
    expect(getByText('get_weather')).toBeTruthy();
  });
});
