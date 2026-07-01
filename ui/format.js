/* Shared number/money display formatting for the dyson + swarm UIs.
 *
 * Money rendering existed in three diverged copies (dyson's rendered
 * `$-1.23`, had no balance rounding, and stopped compacting at "M").
 * Both apps show the same figures — one implementation keeps them
 * rendering identically.
 */

/// Sub-cent-precise USD for per-call costs: `$0.000123`, `-$1.23`,
/// `$12.34`. Null/undefined/NaN render as `$0.00`.
export function formatUsd(value) {
  if (value === null || value === undefined || Number.isNaN(Number(value))) return '$0.00';
  const n = Number(value);
  const sign = n < 0 ? '-' : '';
  const abs = Math.abs(n);
  if (abs > 0 && abs < 0.01) return `${sign}$${trimFixed(abs, abs < 0.0001 ? 6 : 4)}`;
  return `${sign}$${abs.toFixed(2)}`;
}

/// Money for a wallet/cap balance: rounded to whole cents (so floating dust
/// never shows as "-$0.00"), with a real minus sign for debt. Use this for
/// balances; `formatUsd` keeps sub-cent precision for tiny per-call costs.
export function formatBalance(value) {
  const cents = Math.round((Number(value) || 0) * 100);
  if (cents === 0) return '$0.00';
  const abs = (Math.abs(cents) / 100).toFixed(2);
  return cents < 0 ? `−$${abs}` : `$${abs}`;
}

/// Compact token/count display: `950`, `1.5k`, `2.3M`, `1.1B`.
export function formatTokens(value) {
  const n = Number(value || 0);
  if (n >= 1_000_000_000) return `${trimFixed(n / 1_000_000_000, 1)}B`;
  if (n >= 1_000_000) return `${trimFixed(n / 1_000_000, 1)}M`;
  if (n >= 1_000) return `${trimFixed(n / 1_000, 1)}k`;
  return String(Math.max(0, Math.round(n)));
}

/// Locale-grouped integer: `1,234,567`.
export function formatCount(value) {
  return new Intl.NumberFormat('en-US').format(Number(value || 0));
}

/// Byte size: `—` for absent, `512 B`, `1.5 KB`, `12 MB`, `3.1 GB`.
export function formatBytes(n) {
  if (!Number.isFinite(n) || n <= 0) return '—';
  const units = ['B', 'KB', 'MB', 'GB'];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(v < 10 && i > 0 ? 1 : 0)} ${units[i]}`;
}

/// Compact run-time from seconds: "45s", "12m", "3.4h", "210h".
export function formatDuration(seconds) {
  const s = Math.max(0, Math.round(Number(seconds || 0)));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.round(s / 60)}m`;
  const h = s / 3600;
  return h < 100 ? `${trimFixed(h, 1)}h` : `${Math.round(h)}h`;
}

function trimFixed(value, digits) {
  return Number(value).toFixed(digits).replace(/\.0+$|(\.\d*?)0+$/u, '$1');
}
