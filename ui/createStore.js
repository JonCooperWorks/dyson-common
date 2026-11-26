/* Minimal subscribe / getSnapshot / dispatch store.
 *
 * Built for React's useSyncExternalStore: one frozen snapshot, listeners
 * fire synchronously on change, reducers that return the same reference
 * short-circuit (no allocation, no notification).
 *
 * Snapshots are deep-frozen in development and tests, where a mutation
 * in place should throw; a production build skips the walk (every
 * dispatch would otherwise visit the whole state). `{ freeze }` overrides.
 */

// Bundlers (Vite, esbuild) replace `process.env.NODE_ENV` with a literal,
// so this folds to a constant in a production build. Without a bundler
// `process` is undefined and nothing is frozen.
function freezeByDefault() {
  try {
    return process.env.NODE_ENV !== 'production';
  } catch {
    return false;
  }
}

export function deepFreeze(obj) {
  if (obj === null || typeof obj !== 'object') return obj;
  if (Object.isFrozen(obj)) return obj;
  Object.freeze(obj);
  for (const key of Object.keys(obj)) {
    const v = obj[key];
    if (v !== null && typeof v === 'object' && !Object.isFrozen(v)) deepFreeze(v);
  }
  return obj;
}

export function createStore(initial, { freeze = freezeByDefault() } = {}) {
  const settle = freeze ? deepFreeze : (value) => value;
  let snapshot = settle(initial);
  const listeners = new Set();

  const getSnapshot = () => snapshot;

  const subscribe = (fn) => {
    listeners.add(fn);
    return () => { listeners.delete(fn); };
  };

  // Reducer contract: return the SAME reference to signal "no change" —
  // cheaper than a deep-equal check and surfaces bugs where the caller
  // thought they were mutating in place.
  const dispatch = (reducer) => {
    const next = reducer(snapshot);
    if (next === snapshot) return;
    snapshot = settle(next);
    for (const fn of listeners) fn();
  };

  return { getSnapshot, subscribe, dispatch };
}
