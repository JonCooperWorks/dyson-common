import { describe, expect, it } from 'vitest';
import {
  formatBalance,
  formatBytes,
  formatCount,
  formatDuration,
  formatTokens,
  formatUsd,
} from './format.js';

describe('formatUsd', () => {
  it('renders sub-cent precision for tiny costs', () => {
    expect(formatUsd(0.00005)).toBe('$0.00005');
    expect(formatUsd(0.000123)).toBe('$0.0001');
    expect(formatUsd(0.0045)).toBe('$0.0045');
  });
  it('renders cents for ordinary amounts and signs negatives', () => {
    expect(formatUsd(12.345)).toBe('$12.35');
    // The dyson copy used to render "$-1.23".
    expect(formatUsd(-1.23)).toBe('-$1.23');
  });
  it('is defensive about nullish/NaN', () => {
    expect(formatUsd(null)).toBe('$0.00');
    expect(formatUsd(undefined)).toBe('$0.00');
    expect(formatUsd('nope')).toBe('$0.00');
  });
});

describe('formatBalance', () => {
  it('rounds floating dust away from "-$0.00"', () => {
    expect(formatBalance(-0.0001)).toBe('$0.00');
    expect(formatBalance(-1.005)).toBe('−$1.00');
    expect(formatBalance(20)).toBe('$20.00');
  });
});

describe('compact numbers', () => {
  it('formatTokens spans k/M/B', () => {
    expect(formatTokens(950)).toBe('950');
    expect(formatTokens(1500)).toBe('1.5k');
    expect(formatTokens(2_300_000)).toBe('2.3M');
    expect(formatTokens(1_100_000_000)).toBe('1.1B');
  });
  it('formatCount groups digits', () => {
    expect(formatCount(1234567)).toBe('1,234,567');
  });
  it('formatBytes spans B → GB', () => {
    expect(formatBytes(0)).toBe('—');
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(3.1 * 1024 ** 3)).toBe('3.1 GB');
  });
  it('formatDuration compacts seconds', () => {
    expect(formatDuration(45)).toBe('45s');
    expect(formatDuration(720)).toBe('12m');
    expect(formatDuration(3.4 * 3600)).toBe('3.4h');
  });
});
