/* How a table orders its rows: the vocabulary shared by the sortable
 * heads (Sorting.jsx) and lists sorted in the browser. A server-sorted
 * list takes the same `sort=` value (`-col` for descending), so a page
 * can sort its whole list, not just the page on screen, either way.
 *
 * A table's columns are `{ key: [label, kind, first?] }`. `kind` is
 * 'time', 'num', 'text' or 'order' (addresses, versions and other
 * ordinals): times and numbers start largest first, text and ordinals
 * smallest first. A column the unsorted list is already ordered by
 * says `first: 'asc'` (or 'desc'), so its first click shows something
 * new. */

export const SORT_NATURAL = { time: true, num: true, text: false, order: false };
export const SORT_DIRECTION = {
  time: ['oldest first', 'newest first'],
  num: ['lowest first', 'highest first'],
  text: ['A → Z', 'Z → A'],
  order: ['lowest first', 'highest first'],
};

/// Whether a first click sorts `column` largest first.
export const firstDesc = column => (column?.[2] ? column[2] === 'desc' : SORT_NATURAL[column?.[1]] ?? false);

/// `"-col"` → `{ key: 'col', desc: true }`; empty → null.
export function parseSort(value) {
  const text = (value || '').trim();
  if (!text || text === '-') return null;
  return text.startsWith('-') ? { key: text.slice(1), desc: true } : { key: text, desc: false };
}

/// The inverse of `parseSort`: null → ''.
export const sortParam = sort => (sort ? `${sort.desc ? '-' : ''}${sort.key}` : '');

/// The two ways a column sorts, its natural one first, as
/// `[[desc, words], …]`.
export function sortWays(kind) {
  const [up, down] = SORT_DIRECTION[kind] || SORT_DIRECTION.text;
  return SORT_NATURAL[kind] ? [[true, down], [false, up]] : [[false, up], [true, down]];
}

/// Orders `rows` like a server would: `value(row, key)` gives a number,
/// a string or null; strings compare case-blind, nulls go last either
/// way, and equal rows keep their order.
export function sortRows(rows, sort, value) {
  if (!sort) return rows;
  const keyed = rows.map((row, i) => {
    const v = value(row, sort.key);
    return [v == null || v === '' || Number.isNaN(v) ? null : typeof v === 'string' ? v.toLowerCase() : v, i, row];
  });
  keyed.sort((a, b) => {
    if (a[0] === null || b[0] === null) return a[0] === b[0] ? a[1] - b[1] : a[0] === null ? 1 : -1;
    const order = a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0;
    return (sort.desc ? -order : order) || a[1] - b[1];
  });
  return keyed.map(k => k[2]);
}
