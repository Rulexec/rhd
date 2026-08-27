import { describe, it, afterEach, expect } from 'vitest';
import { makeAutoObservable } from 'mobx';
import { render, waitFor, cleanup } from '@testing-library/svelte';
import MobxObservableHarness from './mobxObservableHarness.svelte';
import type { ComponentProps } from 'svelte';

afterEach(() => {
  cleanup();
});

class TestStore {
  value = 42;

  constructor() {
    makeAutoObservable(this);
  }

  setValue(nextValue: number): void {
    this.value = nextValue;
  }
}

describe('mobxObservable', () => {
  it('should return the initial value when used inside a component', () => {
    const store = new TestStore();
    const { getByTestId } = render(MobxObservableHarness, { store } satisfies ComponentProps<typeof MobxObservableHarness>);

    expect(getByTestId('observed-value').textContent).toBe('42');
  });

  it('should update the rendered value when the MobX observable changes', async () => {
    const store = new TestStore();
    const { getByTestId } = render(MobxObservableHarness, { store } satisfies ComponentProps<typeof MobxObservableHarness>);

    expect(getByTestId('observed-value').textContent).toBe('42');

    store.setValue(100);

    await waitFor(() => {
      expect(getByTestId('observed-value').textContent).toBe('100');
    });
  });
});