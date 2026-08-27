import { describe, it, expect } from 'vitest';
import { flow } from 'mobx';
import { yieldPromise } from './async.js';

describe('yieldPromise', () => {
  it('should resolve promise value', async () => {
    const testFlow = flow(function* () {
      const result = yield* yieldPromise(Promise.resolve(42));
      return result;
    });

    const result = await testFlow();
    expect(result).toBe(42);
  });

  it('should work with synchronous values', async () => {
    const testFlow = flow(function* () {
      const result = yield* yieldPromise(42);
      return result;
    });

    const result = await testFlow();
    expect(result).toBe(42);
  });
});