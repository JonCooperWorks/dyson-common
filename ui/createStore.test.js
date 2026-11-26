/* Tests for the shared subscribe/dispatch store. */
import { afterEach, describe, expect, test, vi } from 'vitest';
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

describe('createStore freezing (PERF-07)', () => {
  afterEach(() => vi.unstubAllEnvs());

  test('does not deep-freeze snapshots in a production build', () => {
    vi.stubEnv('NODE_ENV', 'production');
    const store = createStore({ a: { b: 1 } });
    expect(Object.isFrozen(store.getSnapshot())).toBe(false);
    store.dispatch((s) => ({ ...s, a: { b: 2 } }));
    expect(Object.isFrozen(store.getSnapshot().a)).toBe(false);
  });

  test('freezes in dev and test, and an explicit option wins', () => {
    vi.stubEnv('NODE_ENV', 'development');
    expect(Object.isFrozen(createStore({ n: 0 }).getSnapshot())).toBe(true);
    expect(Object.isFrozen(createStore({ n: 0 }, { freeze: false }).getSnapshot())).toBe(false);
    vi.stubEnv('NODE_ENV', 'production');
    expect(Object.isFrozen(createStore({ n: 0 }, { freeze: true }).getSnapshot())).toBe(true);
  });
});
