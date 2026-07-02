/* Tests for the shared subscribe/dispatch store. */
import { describe, expect, test, vi } from 'vitest';
import { createStore, deepFreeze } from './createStore.js';

describe('deepFreeze', () => {
  test('freezes nested objects', () => {
    const o = deepFreeze({ a: { b: 1 }, list: [{ c: 2 }] });
    expect(Object.isFrozen(o)).toBe(true);
    expect(Object.isFrozen(o.a)).toBe(true);
    expect(Object.isFrozen(o.list)).toBe(true);
    expect(Object.isFrozen(o.list[0])).toBe(true);
  });

  test('passes primitives through', () => {
    expect(deepFreeze(3)).toBe(3);
    expect(deepFreeze(null)).toBe(null);
  });
});

describe('createStore', () => {
  test('exposes a frozen initial snapshot', () => {
    const store = createStore({ n: 0 });
    expect(store.getSnapshot()).toEqual({ n: 0 });
    expect(Object.isFrozen(store.getSnapshot())).toBe(true);
  });

  test('dispatch swaps the snapshot and notifies subscribers', () => {
    const store = createStore({ n: 0 });
    const listener = vi.fn();
    store.subscribe(listener);

    store.dispatch((s) => ({ ...s, n: s.n + 1 }));
    expect(store.getSnapshot()).toEqual({ n: 1 });
    expect(Object.isFrozen(store.getSnapshot())).toBe(true);
    expect(listener).toHaveBeenCalledTimes(1);
  });

  test('reducer returning the same reference is a no-op (no notification)', () => {
    const store = createStore({ n: 0 });
    const listener = vi.fn();
    store.subscribe(listener);

    store.dispatch((s) => s);
    expect(listener).not.toHaveBeenCalled();
  });

  test('unsubscribe stops further notifications', () => {
    const store = createStore({ n: 0 });
    const listener = vi.fn();
    const unsub = store.subscribe(listener);
    unsub();
    store.dispatch((s) => ({ ...s, n: 1 }));
    expect(listener).not.toHaveBeenCalled();
  });
});
