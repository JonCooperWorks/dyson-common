/* React hook factory for a createStore() store.
 *
 * Wraps useSyncExternalStore with a cached selector so subscribers only
 * re-render when their slice actually changes by value.  The cache is
 * tolerant of React's double-invoke-in-strict-mode: the ref closure holds
 * the last snapshot reference, and identity equality on the full snapshot
 * short-circuits before the selector runs.
 */

import { useSyncExternalStore, useRef } from 'react';

const identity = (s) => s;

// Bind a store (from createStore) to a useAppState(selector?) hook. The
// store is captured once at call time so each app can create its own hook
// against its own store without this module importing any app state.
export function createUseAppState(store) {
  return function useAppState(selector) {
    const sel = selector || identity;
    const cacheRef = useRef(null);

    // getSnapshot is called during render; the cache holds the last
    // (fullSnapshot, selectedValue) pair so repeated calls for the same
    // store state return the same selected reference — that's what lets
    // React skip the re-render when the selected slice hasn't changed.
    const getSnapshot = () => {
      const snap = store.getSnapshot();
      const cache = cacheRef.current;
      if (cache && cache.snap === snap) return cache.selected;
      const selected = sel(snap);
      if (cache && Object.is(cache.selected, selected)) {
        cacheRef.current = { snap, selected: cache.selected };
        return cache.selected;
      }
      cacheRef.current = { snap, selected };
      return selected;
    };

    return useSyncExternalStore(store.subscribe, getSnapshot, getSnapshot);
  };
}
