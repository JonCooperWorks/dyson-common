import { describe, expect, test } from 'vitest';
import {
  formatAbsolute, formatCount, formatDay, formatDuration, formatDurationMs, formatFull,
  formatMoney, formatRelative, midTruncate, toDate,
} from './fmt.js';

// Local wall-clock instants, so the expectations hold in any time zone.
const NOW = new Date(2026, 10, 15, 13, 6, 48); // Sun 15 Nov 2026 13:06:48
const ago = secs => new Date(NOW.getTime() - secs * 1000);
const ahead = secs => new Date(NOW.getTime() + secs * 1000);

describe('dates (spec 4.5)', () => {
  test('accepts seconds, milliseconds, ISO strings and Dates', () => {
    const secs = Math.floor(NOW.getTime() / 1000);
    expect(toDate(secs).getTime()).toBe(secs * 1000);
    expect(toDate(secs * 1000).getTime()).toBe(secs * 1000);
    expect(toDate(NOW.toISOString()).getTime()).toBe(NOW.getTime());
    expect(toDate(null)).toBeNull();
    expect(toDate('nonsense')).toBeNull();
  });

  test('relative times step from minutes to weekdays to dates', () => {
    const now = NOW;
    expect(formatRelative(ago(10), { now })).toBe('Just now');
    expect(formatRelative(ago(12 * 60), { now })).toBe('12 min ago');
    expect(formatRelative(ago(20 * 3600), { now })).toBe('20 h ago');
    expect(formatRelative(new Date(2026, 10, 9, 14, 2), { now })).toBe('Mon 14:02');
    expect(formatRelative(new Date(2026, 8, 9, 14, 2), { now })).toBe('9 Sep');
    expect(formatRelative(new Date(2025, 10, 9, 14, 2), { now })).toBe('9 Nov 2025');
  });

  test('future times read "in 3 h", then "tomorrow 09:00"', () => {
    const now = NOW;
    expect(formatRelative(ahead(12 * 60), { now })).toBe('in 12 min');
    expect(formatRelative(ahead(3 * 3600), { now })).toBe('in 3 h');
    expect(formatRelative(new Date(2026, 10, 16, 9, 0), { now })).toBe('tomorrow 09:00');
    expect(formatRelative(new Date(2026, 10, 19, 9, 0), { now })).toBe('Thu 09:00');
  });

  test('absolute log-style times are day-month and 24 h, never 17/10', () => {
    expect(formatAbsolute(new Date(2026, 9, 17, 13, 6, 48), { now: NOW })).toBe('17 Oct 13:06:48');
    expect(formatAbsolute(new Date(2025, 9, 17, 9, 5, 1), { now: NOW })).toBe('17 Oct 2025 09:05:01');
    expect(formatAbsolute(new Date(2026, 9, 17, 13, 6, 48), { now: NOW, seconds: false })).toBe('17 Oct 13:06');
    expect(formatDay(new Date(2026, 10, 9), { now: NOW })).toBe('9 Nov');
  });

  test('the tooltip carries local time and UTC', () => {
    const text = formatFull(NOW);
    expect(text).toMatch(/^Sun 15 Nov 2026 13:06:48 · \d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} UTC$/);
  });

  test('missing values are a dash', () => {
    expect(formatRelative(null)).toBe('—');
    expect(formatAbsolute(undefined)).toBe('—');
    expect(formatAbsolute(0)).toBe('—');
  });
});

describe('durations, money, counts, IDs', () => {
  test('durations use two units at most', () => {
    expect(formatDuration(0.4)).toBe('0.4 s');
    expect(formatDuration(12)).toBe('12 s');
    expect(formatDuration(120)).toBe('2 min');
    expect(formatDuration(250)).toBe('4 min 10 s');
    expect(formatDuration(72 * 60)).toBe('1 h 12 min');
    expect(formatDuration(5 * 86400 + 16 * 3600 + 59)).toBe('5 d 16 h');
    expect(formatDurationMs(420)).toBe('420 ms');
    expect(formatDuration(-1)).toBe('—');
  });

  test('money has two decimals, separators, and a floor under a cent', () => {
    expect(formatMoney(1234.5)).toBe('$1,234.50');
    expect(formatMoney(0)).toBe('$0.00');
    expect(formatMoney(0.001)).toBe('<$0.01');
    expect(formatMoney(null)).toBe('—');
    expect(formatMoney(3, { ownPlan: true })).toBe('Own plan');
  });

  test('counts group thousands and go compact from 10,000', () => {
    expect(formatCount(1337)).toBe('1,337');
    expect(formatCount(12400)).toBe('12.4k');
    expect(formatCount(3_100_000)).toBe('3.1M');
    expect(formatCount(0)).toBe('0');
  });

  test('IDs are middle-truncated', () => {
    const out = midTruncate('fertile-otter-0b1c2d3e4b26de', 16);
    expect(out).toHaveLength(16);
    expect(out.startsWith('fertile-')).toBe(true);
    expect(out.endsWith('4b26de')).toBe(true);
    expect(midTruncate('short', 16)).toBe('short');
  });
});
