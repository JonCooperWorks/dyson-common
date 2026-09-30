import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Combobox } from './Combobox.jsx';
import { SORT_DIRECTION, firstDesc, parseSort, sortParam, sortWays } from './sort.js';

/* Sortable table heads that order the whole list, not the page on screen.
 *
 * The sort is one string (`"-col"` for descending, '' for unsorted) the
 * app keeps where it likes — usually the URL, so a sorted view is a link.
 * `useSorter` turns that string and a setter into the `sorter` every
 * piece here takes:
 *
 *   const sorter = useSorter(COLUMNS, { value: query.get('sort'), onChange: v => setQuery({ sort: v }) });
 *   <SortTh sorter={sorter} keys={['latency', 'ttfb']}>Latency</SortTh>
 *   <SortPill sorter={sorter} />      // in the list's foot: what it's sorted by, and a way back
 *   <SortPicker sorter={sorter} />    // phones: the same choice as a picker above a card list
 *
 * A click sorts a column its natural way first, then the other way, then
 * back to unsorted. A column with several keys (latency / time to first
 * byte) gets a menu for the rest. */

export function useSorter(columns, { value, onChange, fallback = 'newest first' }) {
  const parsed = parseSort(value);
  const active = parsed && columns[parsed.key] ? parsed : null;
  const set = next => onChange(sortParam(next));
  const cycle = key => {
    const natural = firstDesc(columns[key]);
    if (active?.key !== key) return set({ key, desc: natural });
    if (active.desc === natural) return set({ key, desc: !natural });
    return set(null);
  };
  const describe = sort => {
    const [label, kind] = columns[sort.key] || [sort.key, 'text'];
    return { label, direction: (SORT_DIRECTION[kind] || SORT_DIRECTION.text)[sort.desc ? 1 : 0] };
  };
  return { columns, active, param: sortParam(active), set, cycle, describe, fallback };
}

// Two stacked carets; the live one lights up.
export function SortGlyph({ dir }) {
  return <svg className={`dc-sort-glyph ${dir || ''}`.trim()} width="8" height="12" viewBox="0 0 8 12" aria-hidden="true"><path className="up" d="M4 1.5 7 5H1z" /><path className="down" d="M4 10.5 1 7h6z" /></svg>;
}

const Stroke = ({ d, size }) => <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={d} /></svg>;
const CLOSE = 'M18 6L6 18M6 6l12 12';
const DOWN = 'M6 9l6 6 6-6';

// A sortable column head. `keys` are the column's sort keys, first the
// one a click uses. Sorted on another of its keys, the head names that
// key instead.
export function SortTh({ sorter, keys, children, className = '', title }) {
  const [open, setOpen] = useState(false);
  const button = useRef(null);
  const onKey = keys.find(k => sorter.active?.key === k);
  const dir = onKey ? (sorter.active.desc ? 'desc' : 'asc') : null;
  const primary = onKey || keys[0];
  const aside = onKey && onKey !== keys[0] ? sorter.columns[onKey][0] : null;
  const described = onKey ? sorter.describe(sorter.active) : null;
  const hint = described
    ? `Sorted by ${described.label.toLowerCase()}, ${described.direction}. Click to ${sorter.active.desc === firstDesc(sorter.columns[onKey]) ? 'reverse' : `go back to ${sorter.fallback}`}.`
    : `Sort by ${sorter.columns[primary][0].toLowerCase()}`;
  return <th className={`dc-sortable ${onKey ? 'sorted' : ''} ${className}`.trim()} aria-sort={dir === 'asc' ? 'ascending' : dir === 'desc' ? 'descending' : 'none'} title={title}>
    <span className="dc-sort-head">
      <button type="button" className="dc-sort-btn" onClick={() => sorter.cycle(primary)} title={hint}>
        <span className="dc-sort-label">{aside ? <span className="dc-sort-aside">{aside}</span> : children}</span>
        <SortGlyph dir={dir} />
      </button>
      {keys.length > 1 && <button type="button" ref={button} className="dc-sort-more" aria-haspopup="menu" aria-expanded={open} aria-label={`More ways to sort ${typeof children === 'string' ? children : 'this column'}`} title="Sort by…" onClick={() => setOpen(o => !o)}><Stroke d={DOWN} size={10} /></button>}
    </span>
    {open && <SortMenu anchor={button} sorter={sorter} keys={keys} onClose={() => setOpen(false)} />}
  </th>;
}

// The menu under a column with several sort keys: each key, either way.
// Portalled so a scrolling table cannot clip it; ↑/↓ move, Esc closes.
function SortMenu({ anchor, sorter, keys, onClose }) {
  const menu = useRef(null);
  const [at, setAt] = useState(null);
  useLayoutEffect(() => {
    const place = () => { const r = anchor.current?.getBoundingClientRect(); if (r) setAt({ top: r.bottom + 6, left: Math.max(8, Math.min(r.right - 232, window.innerWidth - 240)) }); };
    place();
    window.addEventListener('resize', place); window.addEventListener('scroll', place, true);
    return () => { window.removeEventListener('resize', place); window.removeEventListener('scroll', place, true); };
  }, [anchor]);
  useEffect(() => {
    const onDown = event => { if (!menu.current?.contains(event.target) && !anchor.current?.contains(event.target)) onClose(); };
    const onKey = event => {
      if (event.key === 'Escape') { event.stopPropagation(); onClose(); anchor.current?.focus(); }
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        const items = [...(menu.current?.querySelectorAll('button') || [])];
        const i = items.indexOf(document.activeElement);
        items[(i + (event.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length]?.focus();
      }
    };
    document.addEventListener('mousedown', onDown); document.addEventListener('keydown', onKey);
    menu.current?.querySelector('button[aria-checked="true"], button')?.focus();
    return () => { document.removeEventListener('mousedown', onDown); document.removeEventListener('keydown', onKey); };
  }, [anchor, onClose]);
  const pick = sort => { sorter.set(sort); onClose(); };
  return createPortal(<div ref={menu} className="dc-sort-menu" role="menu" style={at ? { top: at.top, left: at.left } : { visibility: 'hidden' }}>
    <div className="dc-sort-menu-head">Sort by</div>
    {keys.map(key => {
      const [label, kind] = sorter.columns[key];
      return <div key={key} className="dc-sort-menu-row">
        <span className="dc-sort-menu-label">{label}</span>
        <span className="dc-sort-menu-ways">{sortWays(kind).map(([desc, text]) => {
          const on = sorter.active?.key === key && sorter.active.desc === desc;
          return <button type="button" key={String(desc)} role="menuitemradio" aria-checked={on} className={on ? 'on' : ''} onClick={() => pick(on ? null : { key, desc })}><SortGlyph dir={desc ? 'desc' : 'asc'} />{text}</button>;
        })}</span>
      </div>;
    })}
    {sorter.active && <button type="button" className="dc-sort-menu-reset" onClick={() => pick(null)}><Stroke d={CLOSE} size={11} />Back to {sorter.fallback}</button>}
  </div>, document.body);
}

// What the list is sorted by, with a way back. Renders nothing unsorted.
export function SortPill({ sorter }) {
  if (!sorter?.active) return null;
  const { label, direction } = sorter.describe(sorter.active);
  return <span className="dc-sort-pill" role="status"><SortGlyph dir={sorter.active.desc ? 'desc' : 'asc'} /><span>Sorted by <b>{label}</b> · {direction}</span><button type="button" onClick={() => sorter.set(null)} aria-label={`Stop sorting; back to ${sorter.fallback}`} title={`Back to ${sorter.fallback}`}><Stroke d={CLOSE} size={11} /></button></span>;
}

// Phones fold a table into cards with no heads to click, so the same
// choice is a picker above the list there (shown below 640px, or
// always with `always`). `boxClassName` joins the picker's own box, for
// an app that dresses its comboboxes.
export function SortPicker({ sorter, always = false, boxClassName = '' }) {
  const options = [{ value: '', label: `Default · ${sorter.fallback}` }, ...Object.entries(sorter.columns).flatMap(([key, [label, kind]]) =>
    sortWays(kind).map(([desc, text]) => ({ value: sortParam({ key, desc }), label: `${label} · ${text}` })))];
  return <div className={`dc-sort-picker ${always ? 'always' : ''}`.trim()}><span>Sort</span><div className={`dc-sort-picker-box ${boxClassName}`.trim()}><Combobox ariaLabel="Sort by" options={options} value={sorter.param} onSelect={option => sorter.set(parseSort(option.value))} onClear={() => sorter.set(null)} /></div></div>;
}
