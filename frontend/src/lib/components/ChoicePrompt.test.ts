import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import ChoicePrompt from './ChoicePrompt.svelte';
import type { ChoiceToolArgs } from '../api/schemas.js';

const args: ChoiceToolArgs = { question: 'Which plan?', options: ['Alpha', 'Beta'] };

afterEach(cleanup);

describe('ChoicePrompt', () => {
  it('renders the question and one button per option', () => {
    const { getByText, getAllByRole } = render(ChoicePrompt, {
      props: { args, resolved: false, onRespond: vi.fn() }
    });

    expect(getByText('Which plan?')).toBeTruthy();
    // Option buttons plus the manual "Send" button.
    const buttons = getAllByRole('button');
    expect(buttons.length).toBe(3);
    expect(getByText('Alpha')).toBeTruthy();
    expect(getByText('Beta')).toBeTruthy();
    expect(getByText('Send')).toBeTruthy();
  });

  it('calls onRespond with the exact option text when an option is clicked', async () => {
    const onRespond = vi.fn();
    const { getByText } = render(ChoicePrompt, {
      props: { args, resolved: false, onRespond }
    });

    await fireEvent.click(getByText('Alpha'));

    expect(onRespond).toHaveBeenCalledTimes(1);
    expect(onRespond).toHaveBeenCalledWith('Alpha');
  });

  it('submits trimmed manual text; empty input does not respond', async () => {
    const onRespond = vi.fn();
    const { container } = render(ChoicePrompt, {
      props: { args, resolved: false, onRespond }
    });

    const input = container.querySelector('.choice-manual-input') as HTMLInputElement;
    const form = container.querySelector('.choice-manual') as HTMLFormElement;

    // Empty input: submit must not respond.
    await fireEvent.submit(form);
    expect(onRespond).not.toHaveBeenCalled();

    // Whitespace-padded text: submit responds with the trimmed value.
    await fireEvent.input(input, { target: { value: '  go with the beta plan  ' } });
    await fireEvent.submit(form);
    expect(onRespond).toHaveBeenCalledTimes(1);
    expect(onRespond).toHaveBeenCalledWith('go with the beta plan');
  });

  it('disables controls after responding so a second send cannot fire', async () => {
    const onRespond = vi.fn();
    const { container, getByText } = render(ChoicePrompt, {
      props: { args, resolved: false, onRespond }
    });

    await fireEvent.click(getByText('Alpha'));
    await fireEvent.click(getByText('Beta'));

    expect(onRespond).toHaveBeenCalledTimes(1);
    expect(onRespond).toHaveBeenCalledWith('Alpha');
    for (const button of container.querySelectorAll<HTMLButtonElement>('.choice-option')) {
      expect(button.disabled).toBe(true);
    }
  });

  it('renders the resolved answer and no option buttons', () => {
    const { getByText, container } = render(ChoicePrompt, {
      props: { args, resolved: true, answer: 'Beta', onRespond: vi.fn() }
    });

    expect(getByText('Answered:')).toBeTruthy();
    expect(getByText('Beta')).toBeTruthy();
    expect(container.querySelector('.choice-option')).toBeNull();
    expect(container.querySelector('.choice-manual')).toBeNull();
  });

  it('does not respond when disabled', async () => {
    const onRespond = vi.fn();
    const { getByText } = render(ChoicePrompt, {
      props: { args, resolved: false, disabled: true, onRespond }
    });

    await fireEvent.click(getByText('Alpha'));

    expect(onRespond).not.toHaveBeenCalled();
  });
});
