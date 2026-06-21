import React from 'react';
import { useEscapeKey } from './useEscapeKey.js';

// Elements that can hold keyboard focus.  The dialog box itself carries
// tabindex="-1" (so we can focus it when it has no focusable children),
// which this selector deliberately excludes.
const FOCUSABLE_SELECTOR = [
  'a[href]',
  'area[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

function focusableWithin(root) {
  if (!root) return [];
  return Array.prototype.filter.call(
    root.querySelectorAll(FOCUSABLE_SELECTOR),
    (el) => el.tabIndex !== -1 && !el.hasAttribute('disabled'),
  );
}

/**
 * Shared modal shell: a full-screen scrim with a centred dialog box.
 *
 * Owns the cross-cutting modal behaviour so every modal gets it the same
 * way instead of re-implementing it: Escape dismisses, clicking the scrim
 * (but not the dialog itself) dismisses, the box carries the dialog a11y
 * roles, focus moves into the dialog on open and is trapped there (Tab /
 * Shift+Tab cycle within it instead of walking into the page behind the
 * scrim), and focus is restored to the opener on close.  Callers pass
 * their header / body / actions as children and keep their own markup.
 *
 * Pass `closeOnScrimClick={false}` for flows where an accidental
 * backdrop click should not discard work.  `className` / `scrimClassName`
 * override the default classes so a modal with its own look (e.g. a
 * command palette) can still reuse the shared dismiss behaviour.
 */
export function Modal({
  onClose,
  label,
  labelledBy,
  className = 'modal',
  scrimClassName = 'modal-scrim',
  closeOnScrimClick = true,
  children,
}) {
  useEscapeKey(onClose);
  const dialogRef = React.useRef(null);

  // Remember what had focus when the modal opened so we can restore it on
  // close.  Captured during render (not in an effect): a child that
  // autofocuses does so via its own mount effect, which React runs before
  // the parent's — by then `document.activeElement` is already the child,
  // not the real opener.  Reading it at render time predates all that.
  const openerRef = React.useRef(undefined);
  if (openerRef.current === undefined) {
    openerRef.current =
      typeof document !== 'undefined' ? document.activeElement : null;
  }

  // Move focus into the dialog on open (unless a child already grabbed it,
  // e.g. an autofocused search input), and restore it to the opener on
  // close/unmount.
  React.useEffect(() => {
    const dialog = dialogRef.current;
    if (dialog && !dialog.contains(document.activeElement)) {
      const focusables = focusableWithin(dialog);
      (focusables[0] || dialog).focus();
    }
    return () => {
      const opener = openerRef.current;
      if (opener && opener.isConnected && typeof opener.focus === 'function') {
        opener.focus();
      }
    };
  }, []);

  const onScrimClick = (e) => {
    if (closeOnScrimClick && e.target === e.currentTarget) onClose?.();
  };

  // Trap Tab/Shift+Tab so focus cycles within the dialog instead of
  // escaping to the page behind the scrim.  Only the wrap-around (and any
  // stray focus outside the dialog) is intercepted; tabbing between the
  // dialog's own controls keeps the browser's native order.
  const onKeyDown = (e) => {
    if (e.key !== 'Tab') return;
    const dialog = dialogRef.current;
    if (!dialog) return;
    const focusables = focusableWithin(dialog);
    if (focusables.length === 0) {
      // Nothing to land on — keep focus on the dialog box.
      e.preventDefault();
      return;
    }
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    const active = document.activeElement;
    if (e.shiftKey) {
      if (active === first || !dialog.contains(active)) {
        e.preventDefault();
        last.focus();
      }
    } else if (active === last || !dialog.contains(active)) {
      e.preventDefault();
      first.focus();
    }
  };

  return (
    <div className={scrimClassName} onClick={onScrimClick}>
      <div
        ref={dialogRef}
        className={className}
        role="dialog"
        aria-modal="true"
        aria-label={label}
        aria-labelledby={labelledBy}
        tabIndex={-1}
        onKeyDown={onKeyDown}
      >
        {children}
      </div>
    </div>
  );
}
