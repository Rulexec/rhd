/**
 * Utility for typed async generators with MobX flow.
 * Allows deducing types from yield promises.
 *
 * Usage:
 * ```typescript
 * *myFlow() {
 *   const result = yield* yieldPromise(fetchData());
 *   // result is properly typed
 * }
 * ```
 */
export function* yieldPromise<T>(
  promise: T | Promise<T>,
): Generator<unknown, T, unknown> {
  return (yield promise) as T;
}