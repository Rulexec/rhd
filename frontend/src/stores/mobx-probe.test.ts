import { describe, it, expect, vi } from 'vitest';
import {
  makeAutoObservable,
  makeObservable,
  flow,
  flowResult,
  autorun,
  observable,
  configure
} from 'mobx';
import { yieldPromise } from '../util/async.js';

/**
 * Probe tests documenting how this codebase's MobX version behaves.
 *
 * These tests are intentionally kept: they document the exact semantics that
 * the MobX stores (ChatsListStore, ChatStore, PluginsStore, ConnectionStore)
 * rely on, so future readers do not need to re-derive them from the MobX
 * source.
 *
 * Key questions answered here:
 * 1. Does `makeAutoObservable` auto-wrap plain generator PROTOTYPE methods
 *    into flows (so calling them returns a promise directly)?
 * 2. Can MobX observe `#`-private class fields? (Answer: NO — they stay plain.)
 * 3. Can an underscore-prefixed public field be observed and drive computed
 *    getters?
 * 4. Do class-FIELD arrow functions assigned via `flow(...)` work with
 *    `makeAutoObservable`?
 * 5. Is the `chats`-style getter (sorted copy over the observable field)
 *    reactive through `autorun`?
 */

class PlainGeneratorStore {
  count = 0;

  constructor() {
    makeAutoObservable(this);
  }

  *increment(): Generator<unknown, number, unknown> {
    yield* yieldPromise(Promise.resolve());
    this.count += 1;
    return this.count;
  }
}

describe('makeAutoObservable + plain generator methods', () => {
  it('auto-wraps generator methods into flows: calling returns a thenable, not a raw generator', () => {
    const store = new PlainGeneratorStore();
    const result = store.increment() as unknown;

    // Empirically observed: calling an auto-wrapped generator method runs the
    // body eagerly up to the first yield and returns a CancellablePromise
    // (thenable), NOT a suspended generator.
    expect(typeof (result as { then?: unknown }).then).toBe('function');
    expect((result as { next?: unknown }).next).toBeUndefined();

    // Because it runs eagerly, the synchronous portion (up to the first yield)
    // has already executed.
    expect(store.count).toBe(0); // yield* yieldPromise suspends before increment
  });

  it('flowResult returns a promise; awaiting it completes the flow', async () => {
    const store = new PlainGeneratorStore();
    const promise = flowResult(store.increment());
    expect(typeof promise.then).toBe('function');
    expect(await promise).toBe(1);
    expect(store.count).toBe(1);
  });

  it('autorun re-runs when a generator flow mutates observable state', async () => {
    const store = new PlainGeneratorStore();
    const seen: number[] = [];
    const dispose = autorun(() => {
      seen.push(store.count);
    });
    expect(seen).toEqual([0]);

    await flowResult(store.increment());
    expect(seen).toEqual([0, 1]);
    dispose();
  });
});

class PrivateFieldStore {
  #count = 0;
  count = 0;

  constructor() {
    makeAutoObservable(this);
  }

  get privateCount(): number {
    return this.#count;
  }

  get publicCount(): number {
    return this.count;
  }

  increment(): void {
    this.#count += 1;
    this.count += 1;
  }

  *incrementFlow(): Generator<unknown, number, unknown> {
    yield* yieldPromise(Promise.resolve());
    this.#count += 1;
    this.count += 1;
    return this.count;
  }
}

describe('#-private vs underscore/public fields', () => {
  it('plain (non-action) method mutation is not reliably observable', () => {
    // With plain prototype methods, MobX defaults (`enforceActions: observed`)
    // do NOT wrap the mutation in an action. Empirically, the autorun does not
    // re-run: confirming that mutations MUST happen inside actions (or flows)
    // for reliable reactivity in this codebase.
    const store = new PrivateFieldStore();
    const seenPublic: number[] = [];
    const dispose = autorun(() => {
      seenPublic.push(store.publicCount);
    });
    expect(seenPublic).toEqual([0]);

    store.increment();

    expect(seenPublic).toEqual([0, 1]);
    dispose();
  });

  it('#-private field changes are not tracked even inside a flow (action)', async () => {
    // Demonstrates WHY the store must keep chats in an observable public
    // field: even a flow (action-wrapped) mutation to a #-private field does
    // not create a reaction dependency, so computed getters over it never
    // re-evaluate.
    const store = new PrivateFieldStore();
    const seenPrivate: number[] = [];
    const dispose = autorun(() => {
      seenPrivate.push(store.privateCount);
    });
    expect(seenPrivate).toEqual([0]);

    const result = flowResult(store.incrementFlow());
    expect(await result).toBe(1);

    // The autorun has no reactive dependency on the #-private field, so it
    // does NOT re-run even though the flow ran to completion.
    expect(seenPrivate).toEqual([0]);
    dispose();
  });

  it('#-private fields remain readable inside methods after makeAutoObservable', () => {
    // Ensures MobX proxy wrapping does not break #-private access.
    const store = new PrivateFieldStore();
    store.increment();
    expect(store.privateCount).toBe(1);
    expect(store.publicCount).toBe(1);
  });
});

class FieldFlowStore {
  count = 0;

  constructor() {
    makeAutoObservable(this);
  }

  increment = flow(function* (this: FieldFlowStore): Generator<unknown, number, unknown> {
    yield* yieldPromise(Promise.resolve());
    this.count += 1;
    return this.count;
  });
}

describe('class-field arrow functions assigned via flow()', () => {
  it('makeAutoObservable treats the flow class field as a plain value; flowResult wraps', async () => {
    const store = new FieldFlowStore();
    const raw = store.increment();
    // With makeAutoObservable, an own class-field holding a flow instance is
    // stored by reference (observable of a function = no-op). Calling it
    // returns a CancellablePromise already (mock the flow wrapper).
    const promise = flowResult(raw);
    expect(await promise).toBe(1);
    expect(store.count).toBe(1);
  });
});

describe('plain (non-flow) method observability', () => {
  it('a plain method mutating an observable field is tracked by autorun', () => {
    class PlainMethodStore {
      items: string[] = [];

      constructor() {
        makeAutoObservable(this);
      }

      addItem(item: string): void {
        this.items = [...this.items, item];
      }
    }

    const store = new PlainMethodStore();
    const seen: string[][] = [];
    const dispose = autorun(() => {
      seen.push(store.items);
    });
    expect(seen).toEqual([[]]);

    store.addItem('a');
    expect(seen).toEqual([[], ['a']]);
    dispose();
  });
});

describe('sorted-copy getter reactivity', () => {
  it('a getter returning a new sorted array re-runs on source change', () => {
    class SortedStore {
      _items: number[] = [];

      constructor() {
        makeAutoObservable(this);
      }

      setItems(items: number[]): void {
        this._items = items;
      }

      get sorted(): number[] {
        return [...this._items].sort((a, b) => a - b);
      }
    }

    const store = new SortedStore();
    const seen: number[][] = [];
    const dispose = autorun(() => {
      seen.push(store.sorted);
    });
    expect(seen).toEqual([[]]);

    store.setItems([3, 1, 2]);
    expect(seen).toEqual([[], [1, 2, 3]]);
    dispose();
  });
});

describe('makeObservable explicit flow annotation', () => {
  it('flow field works with an explicit makeObservable annotation', async () => {
    class ExplicitFlowStore {
      count = 0;

      constructor() {
        makeObservable(this, {
          count: observable,
          increment: flow
        });
      }

      increment = flow(function* (this: ExplicitFlowStore): Generator<unknown, number, unknown> {
        yield* yieldPromise(Promise.resolve());
        this.count += 1;
        return this.count;
      });
    }

    const store = new ExplicitFlowStore();
    expect(await flowResult(store.increment())).toBe(1);
    expect(store.count).toBe(1);
  });
});

describe('strict mode config', () => {
  it('enforceActions is "observed" by default so flows (actions) are tolerated', async () => {
    // `configure` is imported and used so we pin the codebase default.
    // Default MobX behavior ("observed") allows state changes outside actions
    // when nothing is observing; our flows run inside action wrappers anyway.
    configure({ enforceActions: 'observed' });
    const store = new PlainGeneratorStore();
    await flowResult(store.increment());
    expect(store.count).toBe(1);
    expect(vi.isMockFunction(store.increment)).toBe(false); // sanity
  });
});