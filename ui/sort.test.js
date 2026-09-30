import { describe, expect, test } from 'vitest';
import { firstDesc, parseSort, sortParam, sortRows, sortWays } from './sort.js';

describe('sort vocabulary', () => {
  test('reads and writes the direction', () => {
    expect(parseSort('-latency')).toEqual({ key: 'latency', desc: true });
    expect(parseSort('model')).toEqual({ key: 'model', desc: false });
    expect(parseSort('')).toBe(null);
    expect(parseSort('-')).toBe(null);
    expect(sortParam({ key: 'cost', desc: true })).toBe('-cost');
    expect(sortParam(null)).toBe('');
  });

  test('numbers and times start largest first unless the column says otherwise', () => {
    expect(firstDesc(['When', 'time'])).toBe(true);
    expect(firstDesc(['Name', 'text'])).toBe(false);
    expect(firstDesc(['When', 'time', 'asc'])).toBe(false);
    expect(sortWays('num').map(([desc]) => desc)).toEqual([true, false]);
    expect(sortWays('text')).toEqual([[false, 'A → Z'], [true, 'Z → A']]);
  });

  test('puts missing values last either way and keeps ties in order', () => {
    const rows = [{ id: 'a', n: 2 }, { id: 'b', n: null }, { id: 'c', n: 9 }, { id: 'd', n: 2 }];
    const value = (r, key) => r[key];
    expect(sortRows(rows, { key: 'n', desc: false }, value).map(r => r.id)).toEqual(['a', 'd', 'c', 'b']);
    expect(sortRows(rows, { key: 'n', desc: true }, value).map(r => r.id)).toEqual(['c', 'a', 'd', 'b']);
    expect(sortRows(rows, null, value)).toBe(rows);
  });

  test('compares text without case', () => {
    const rows = ['beta', 'Alpha', 'gamma'].map(name => ({ name }));
    expect(sortRows(rows, { key: 'name', desc: false }, (r, k) => r[k]).map(r => r.name)).toEqual(['Alpha', 'beta', 'gamma']);
  });
});
