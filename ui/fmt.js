/* Spec 4.5 display formats shared by dyson and dyson-swarm: dates, durations,
 * money, counts and IDs. Exported from the package as the `fmt` namespace
 * (`fmt.formatRelative(...)`), so the older `formatDuration`/`formatCount`
 * top-level exports keep their meaning for existing callers.
 *
 * Dates are written by hand, not through `Intl`/`toLocale*`: the order must be
 * day-month everywhere ("9 Nov", never "11/9" or "Nov 9"), times are 24 h, and
 * a hand-written formatter costs nothing per row (an options-object
 * `toLocaleTimeString` builds a fresh ICU formatter on every call). */

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const DASH = '—';

const pad = n => String(n).padStart(2, '0');

/// Accept what the API hands us: Unix seconds, epoch milliseconds, an ISO
/// string or a Date. Numbers below 1e11 are seconds (1e11 s is the year 5138;
/// 1e11 ms is 1973). Anything unusable is null.
export function toDate(value) {
  if (value == null || value === '' || value === 0) return null;
  let d;
  if (value instanceof Date) d = value;
  else if (typeof value === 'number') d = new Date(Math.abs(value) < 1e11 ? value * 1000 : value);
  else if (typeof value === 'string' && /^\d+(\.\d+)?$/.test(value.trim())) return toDate(Number(value));
  else d = new Date(value);
  return Number.isNaN(d.getTime()) ? null : d;
}

function nowDate(now) {
  return toDate(now) || new Date();
}

const clock = (d, seconds = false) => `${pad(d.getHours())}:${pad(d.getMinutes())}${seconds ? `:${pad(d.getSeconds())}` : ''}`;
const sameDay = (a, b) => a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
const dayStart = d => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

/// "9 Nov" this year, "9 Nov 2025" otherwise.
export function formatDay(value, { now } = {}) {
  const d = toDate(value);
  if (!d) return DASH;
  const n = nowDate(now);
  return `${d.getDate()} ${MONTHS[d.getMonth()]}${d.getFullYear() === n.getFullYear() ? '' : ` ${d.getFullYear()}`}`;
}

/// Log-style absolute time, local and 24 h: "15 Nov 13:06:48" (year added
/// when it isn't this year). For Activity, Timeline, ledger tool rows and any
/// table where rows are compared with each other.
export function formatAbsolute(value, { now, seconds = true } = {}) {
  const d = toDate(value);
  if (!d) return DASH;
  return `${formatDay(d, { now })} ${clock(d, seconds)}`;
}

/// The full instant for a `title` tooltip: local date-time and UTC.
/// "Sat 15 Nov 2026 13:06:48 · 2026-11-15 18:06:48 UTC"
export function formatFull(value) {
  const d = toDate(value);
  if (!d) return '';
  const local = `${WEEKDAYS[d.getDay()]} ${d.getDate()} ${MONTHS[d.getMonth()]} ${d.getFullYear()} ${clock(d, true)}`;
  const utc = `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())} ${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())} UTC`;
  return `${local} · ${utc}`;
}

/// ISO string for a `<time dateTime>` attribute.
export function isoTime(value) {
  const d = toDate(value);
  return d ? d.toISOString() : undefined;
}

/// Relative time, spec 4.5: "Just now", "12 min ago", "20 h ago",
/// "Mon 14:02", "9 Nov", "9 Nov 2025"; future: "in 12 min", "in 3 h",
/// "tomorrow 09:00", then the same weekday/day forms.
export function formatRelative(value, { now } = {}) {
  const d = toDate(value);
  if (!d) return DASH;
  const n = nowDate(now);
  const delta = (d.getTime() - n.getTime()) / 1000; // > 0 future
  const a = Math.abs(delta);
  if (delta <= 0 || a < 60) {
    if (a < 60) return delta > 0 ? 'in under a minute' : 'Just now';
    if (a < 3600) return `${Math.floor(a / 60)} min ago`;
    if (a < 86400) return `${Math.floor(a / 3600)} h ago`;
    if (a < 7 * 86400) return `${WEEKDAYS[d.getDay()]} ${clock(d)}`;
    return formatDay(d, { now: n });
  }
  if (a < 3600) return `in ${Math.max(1, Math.round(a / 60))} min`;
  if (sameDay(d, n)) return `in ${Math.round(a / 3600)} h`;
  if (dayStart(d) - dayStart(n) <= 86400000 + 3600000) return `tomorrow ${clock(d)}`;
  if (a < 7 * 86400) return `${WEEKDAYS[d.getDay()]} ${clock(d)}`;
  return formatDay(d, { now: n });
}

/// Durations in seconds, two units at most: "0.4 s", "12 s", "2 min",
/// "4 min 10 s", "1 h 12 min", "5 d 16 h".
export function formatDuration(secs) {
  const s = Number(secs);
  if (secs == null || secs === '' || !Number.isFinite(s) || s < 0) return DASH;
  if (s < 10) return `${s < 1 ? s.toFixed(1) : (Math.round(s * 10) / 10).toString()} s`;
  if (s < 60) return `${Math.round(s)} s`;
  const whole = Math.round(s);
  if (whole < 3600) {
    const m = Math.floor(whole / 60);
    const rest = whole % 60;
    return m < 10 && rest ? `${m} min ${rest} s` : `${m} min`;
  }
  if (whole < 86400) {
    const h = Math.floor(whole / 3600);
    const m = Math.floor((whole % 3600) / 60);
    return m ? `${h} h ${m} min` : `${h} h`;
  }
  const days = Math.floor(whole / 86400);
  const h = Math.floor((whole % 86400) / 3600);
  return h ? `${days} d ${h} h` : `${days} d`;
}

/// Milliseconds flavour of formatDuration.
export function formatDurationMs(ms) {
  const n = Number(ms);
  if (ms == null || ms === '' || !Number.isFinite(n)) return DASH;
  if (n < 1000) return `${Math.round(n)} ms`;
  return formatDuration(n / 1000);
}

const GROUPED_2 = new Intl.NumberFormat('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
const GROUPED_0 = new Intl.NumberFormat('en-US', { maximumFractionDigits: 0 });

/// Money: "$1,234.50", "<$0.01" under a cent, "$0.00" for a known zero.
/// `ownPlan` means unmetered ("Own plan"); null/undefined is unknown ("—").
export function formatMoney(usd, { ownPlan = false } = {}) {
  if (ownPlan) return 'Own plan';
  if (usd == null || usd === '') return DASH;
  const n = Number(usd);
  if (!Number.isFinite(n)) return DASH;
  if (n === 0) return '$0.00';
  const sign = n < 0 ? '-' : '';
  const a = Math.abs(n);
  if (a < 0.005) return `${sign}<$0.01`;
  return `${sign}$${GROUPED_2.format(a)}`;
}

/// Counts: "1,337"; compact from 10,000 ("12.4k", "3.1M").
export function formatCount(value) {
  if (value == null || value === '') return DASH;
  const n = Number(value);
  if (!Number.isFinite(n)) return DASH;
  const a = Math.abs(n);
  if (a < 10000) return GROUPED_0.format(n);
  const [div, unit] = a >= 1e9 ? [1e9, 'B'] : a >= 1e6 ? [1e6, 'M'] : [1e3, 'k'];
  const scaled = n / div;
  const text = Math.abs(scaled) >= 100 ? Math.round(scaled).toString() : (Math.round(scaled * 10) / 10).toString();
  return `${text}${unit}`;
}

/// Middle-truncate an ID, host or model to `max` characters, keeping the
/// distinctive tail: "fertile-…-4b26de". Full value belongs in a tooltip.
export function midTruncate(value, max = 20) {
  const s = String(value ?? '');
  if (s.length <= max) return s;
  const keep = Math.max(2, max - 1);
  const head = Math.ceil(keep / 2);
  const tail = keep - head;
  return `${s.slice(0, head)}…${s.slice(s.length - tail)}`;
}

export function shortId(value, { head = 8, tail = 3, max = 12 } = {}) {
  const s = String(value || '');
  if (!s) return DASH;
  return s.length > max ? `${s.slice(0, head)}…${s.slice(-tail)}` : s;
}
