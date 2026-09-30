import { useCallback, useEffect, useState } from 'react';

/// A value kept per browser under `key`, as JSON; `initial` (a value, or
/// a function for one worked out lazily) until it is first set. Storage
/// that throws (private windows, blocked site data) just leaves the value
/// unkept — the hook still works for the page's lifetime.
export function useStored(key, initial) {
  const [value, setValue] = useState(() => {
    try { const raw = localStorage.getItem(key); if (raw != null) return JSON.parse(raw); } catch { /* unkept */ }
    return typeof initial === 'function' ? initial() : initial;
  });
  const set = useCallback(next => setValue(current => {
    const chosen = typeof next === 'function' ? next(current) : next;
    try { localStorage.setItem(key, JSON.stringify(chosen)); } catch { /* unkept */ }
    return chosen;
  }), [key]);
  return [value, set];
}

/// `value`, once it has stopped changing for `delay` ms.
export function useDebounced(value, delay = 250) {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => { const id = setTimeout(() => setDebounced(value), delay); return () => clearTimeout(id); }, [value, delay]);
  return debounced;
}
