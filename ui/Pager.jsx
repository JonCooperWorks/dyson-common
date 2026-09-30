import React, { useState } from 'react';
import { SegmentedControl } from './SegmentedControl.jsx';
import { SortPill } from './Sorting.jsx';
import { formatCount } from './format.js';

/* One paging control for every long list, server-paged or not.
 *
 * `from` is the first row's index among `total` (0-based); `page` and
 * `pages` are 1-based. `pages` may be null when the end is not known yet
 * (a cursor-paged list): the pager then offers "next" while a full page
 * came back. The page number is an input, so a long list is one jump
 * away. The prev/next buttons carry `data-pager="prev|next"` for an app
 * that binds ← and → to them. */

export const PAGE_SIZES = [25, 50, 100, 200];

/// Rows per page, one choice shared by every list and kept per browser.
export function usePageSize({ storageKey = 'dyson-page-size', sizes = PAGE_SIZES, fallback = 50 } = {}) {
  const [size, setSize] = useState(() => {
    try { const n = Number(localStorage.getItem(storageKey)); return sizes.includes(n) ? n : fallback; } catch { return fallback; }
  });
  const choose = n => { setSize(n); try { localStorage.setItem(storageKey, String(n)); } catch { /* kept for this page only */ } };
  return [size, choose];
}

const Chevron = ({ d }) => <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={d} /></svg>;
const FIRST = 'M11 17l-5-5 5-5M18 17l-5-5 5-5';
const PREV = 'M15 18l-6-6 6-6';
const NEXT = 'M9 18l6-6-6-6';
const LAST = 'M13 17l5-5-5-5M6 17l5-5-5-5';

export function Pager({ page, pages = null, from, shown, total, size, onPage, onSize, sizes = PAGE_SIZES, busy = false, noun = ['row', 'rows'], note = null, sorter = null, className = '' }) {
  const [draft, setDraft] = useState(null);
  const last = pages ?? null;
  const canPrev = page > 1;
  const canNext = last == null ? shown >= size : page < last;
  const go = n => { const target = Math.max(1, last == null ? n : Math.min(last, n)); if (target !== page) onPage(target); };
  function jump(event) { event.preventDefault(); const n = Number(draft); setDraft(null); if (Number.isFinite(n) && n >= 1) go(Math.round(n)); }
  const range = shown ? `${formatCount(from + 1)}–${formatCount(from + shown)}` : '0';
  const step = (label, d, onClick, disabled, extra = {}) => <button type="button" className="dc-pager-btn" onClick={onClick} disabled={disabled} title={label} aria-label={label.replace(/\s+[←→]$/, '')} {...extra}><Chevron d={d} /></button>;
  return <nav className={`dc-pager ${className}`.trim()} aria-label="Pages">
    <span className="dc-pager-range">{range} of {formatCount(total)} {total === 1 ? noun[0] : noun[1]}{note && <span className="dc-pager-note"> · {note}</span>}{busy && <span className="dc-pager-note"> · loading…</span>}</span>
    {sorter && <SortPill sorter={sorter} />}
    {(canPrev || canNext) && <span className="dc-pager-steps">
      {step('First page', FIRST, () => go(1), !canPrev)}
      {step('Previous page  ←', PREV, () => go(page - 1), !canPrev, { 'data-pager': 'prev' })}
      <form className="dc-pager-at" onSubmit={jump}><span>Page</span><input type="number" min="1" max={last ?? undefined} value={draft ?? page} onChange={e => setDraft(e.target.value)} onBlur={() => setDraft(null)} aria-label="Page number" />{last != null && <span>of {formatCount(last)}</span>}</form>
      {step('Next page  →', NEXT, () => go(page + 1), !canNext, { 'data-pager': 'next' })}
      {last != null && step('Last page', LAST, () => go(last), !canNext)}
    </span>}
    {onSize && <span className="dc-pager-size"><span>Rows</span><SegmentedControl label="Rows per page" value={size} onChange={onSize} options={sizes.map(n => [n, String(n)])} /></span>}
  </nav>;
}
