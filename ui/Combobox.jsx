import React from 'react';
import { createPortal } from 'react-dom';

/**
 * Reusable searchable single-select dropdown.
 *
 * Unlike a native <input list=datalist> (whose popup filters by the
 * committed label and so hides every other entry once one is chosen) this
 * renders its own list: type to filter, but the full list is always one
 * keystroke (clear the box) away, so no option can become unreachable.
 *
 * The list is portalled to <body> and fixed-positioned against the input,
 * so an ancestor with overflow:hidden/auto (cards, modals, scroll panes)
 * can never clip it. It tracks the input across scroll and resize, and
 * flips above the input when the viewport leaves no room below.
 *
 *   options  : [{ value: string, label: string, hint?: string }]
 *   value    : currently-selected value ('' = none)
 *   onSelect : (option) => void   — fired when an option is chosen
 *   onClear  : () => void         — fired when the box is emptied (optional)
 *
 * Escape closes the list and stops propagating, so a combobox inside a
 * modal closes its own dropdown first rather than dismissing the modal.
 */
export function Combobox({
  options,
  value = '',
  onSelect,
  onClear,
  placeholder = '',
  disabled = false,
  ariaLabel,
}) {
  const opts = options || [];
  const selected = opts.find(o => o.value === value) || null;
  const committedLabel = selected ? selected.label : '';

  const [query, setQuery] = React.useState(committedLabel);
  const [open, setOpen] = React.useState(false);
  const [activeIndex, setActiveIndex] = React.useState(-1);
  const [listPos, setListPos] = React.useState(null);
  const inputRef = React.useRef(null);
  const listId = React.useId();

  React.useEffect(() => { setQuery(committedLabel); }, [committedLabel]);

  // Anchor the portalled list to the input. Fixed positioning is viewport-
  // relative, so recompute whenever anything scrolls (capture phase catches
  // scrolls of inner panes, not just the window) or the window resizes.
  const positionList = React.useCallback(() => {
    const el = inputRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const spaceBelow = window.innerHeight - rect.bottom;
    // 240px = the list's CSS max-height; flip up only when it can't fit
    // below but could fit better above.
    const openUp = spaceBelow < 240 && rect.top > spaceBelow;
    setListPos({
      left: rect.left,
      width: rect.width,
      top: openUp ? undefined : rect.bottom + 2,
      bottom: openUp ? window.innerHeight - rect.top + 2 : undefined,
    });
  }, []);

  React.useLayoutEffect(() => {
    if (!open) return undefined;
    positionList();
    window.addEventListener('scroll', positionList, true);
    window.addEventListener('resize', positionList);
    return () => {
      window.removeEventListener('scroll', positionList, true);
      window.removeEventListener('resize', positionList);
    };
  }, [open, positionList]);

  // While the box still shows the committed label the user hasn't started a
  // new search, so show everything; once they edit, filter by the text.
  const editing = query.trim() !== committedLabel.trim();
  const needle = editing ? query.trim().toLowerCase() : '';
  const visible = needle
    ? opts.filter(o =>
      o.label.toLowerCase().includes(needle) ||
      String(o.value).toLowerCase().includes(needle))
    : opts;

  const exactMatch = (text) => {
    const t = text.trim().toLowerCase();
    if (!t) return null;
    return opts.find(o =>
      o.label.toLowerCase() === t || String(o.value).toLowerCase() === t) || null;
  };

  const commit = (opt) => {
    if (!opt) return;
    onSelect?.(opt);
    setQuery(opt.label);
    setOpen(false);
    setActiveIndex(-1);
  };

  const onChange = (e) => {
    setQuery(e.target.value);
    setOpen(true);
    setActiveIndex(-1);
    if (!e.target.value.trim()) onClear?.();
  };

  const onKeyDown = (e) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setOpen(true);
      setActiveIndex(i => Math.min(i + 1, visible.length - 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setActiveIndex(i => Math.max(i - 1, 0));
    } else if (e.key === 'Enter') {
      if (!open) return;
      e.preventDefault();
      const pick = (activeIndex >= 0 && activeIndex < visible.length)
        ? visible[activeIndex]
        : (exactMatch(query) || (visible.length === 1 ? visible[0] : null));
      if (pick) commit(pick);
    } else if (e.key === 'Escape') {
      if (!open) return;
      // Close our own list instead of letting a parent modal handle Esc.
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
      setQuery(committedLabel);
      setActiveIndex(-1);
    }
  };

  return (
    <div className="combobox">
      <input
        type="text"
        role="combobox"
        aria-expanded={open}
        aria-controls={listId}
        aria-autocomplete="list"
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck={false}
        aria-label={ariaLabel}
        ref={inputRef}
        className="combobox-input"
        value={query}
        placeholder={placeholder}
        disabled={disabled}
        onChange={onChange}
        onFocus={(e) => { setOpen(true); e.target.select(); }}
        onBlur={() => { setOpen(false); setQuery(committedLabel); setActiveIndex(-1); }}
        onKeyDown={onKeyDown}
      />
      {open && !disabled && listPos ? createPortal(
        <ul
          className="combobox-list"
          id={listId}
          role="listbox"
          style={{
            left: listPos.left,
            width: listPos.width,
            top: listPos.top,
            bottom: listPos.bottom,
          }}
          // preventDefault so grabbing the list's scrollbar doesn't blur
          // the input (which would close the list mid-scroll)
          onMouseDown={(e) => e.preventDefault()}
        >
          {visible.length === 0 ? (
            <li className="combobox-empty">no matches</li>
          ) : visible.map((opt, i) => (
            <li
              key={opt.value}
              role="option"
              aria-selected={opt.value === value}
              className={`combobox-option${i === activeIndex ? ' is-active' : ''}`}
              // mousedown (not click) so it fires before the input's blur
              onMouseDown={(e) => { e.preventDefault(); commit(opt); }}
            >
              <span className="combobox-option-label">{opt.label}</span>
              {opt.hint ? <span className="combobox-hint">{opt.hint}</span> : null}
            </li>
          ))}
        </ul>,
        document.body,
      ) : null}
    </div>
  );
}
