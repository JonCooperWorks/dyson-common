import React from 'react';
import { Modal } from './Modal.jsx';

/**
 * Confirmation dialog built on the shared focus-trapped Modal.
 *
 * Replaces native `window.confirm()` on destructive actions. `confirm()`
 * blocks the event loop (which also wedges automated browsers driving the
 * SPA), can't be styled, and is visually inconsistent with the rest of the
 * app's modals. Render this conditionally while an action is pending —
 * `onConfirm` runs the action, `onCancel` dismisses. While `busy` is true
 * the dialog stays put (scrim/Escape dismissals are swallowed) so it can't
 * be closed out from under an in-flight request.
 */
export function ConfirmModal({
  title,
  confirmLabel = 'confirm',
  cancelLabel = 'cancel',
  busy = false,
  onConfirm,
  onCancel,
  children,
}) {
  const onClose = busy ? () => {} : onCancel;
  return (
    <Modal onClose={onClose} label={title}>
      <div className="modal-header">
        <span>{title}</span>
      </div>
      <div className="modal-body confirm-modal-body">
        {children}
      </div>
      <div className="modal-actions">
        <button
          type="button"
          className="btn btn-ghost"
          onClick={onCancel}
          disabled={busy}
        >
          {cancelLabel}
        </button>
        <button
          type="button"
          className="btn btn-primary confirm-modal-confirm"
          onClick={onConfirm}
          disabled={busy}
        >
          {busy ? 'working…' : confirmLabel}
        </button>
      </div>
    </Modal>
  );
}

/**
 * Promise-based wrapper around <ConfirmModal/> — the ergonomic way to
 * replace a native `window.confirm()` without threading open/close state
 * through the host by hand.
 *
 *   const [confirm, confirmModal] = useConfirm();
 *   ...
 *   if (!(await confirm({ title: 'delete X?', message: '…', confirmLabel: 'delete' }))) return;
 *   ...
 *   return <>{children}{confirmModal}</>;
 *
 * `confirm` accepts either a plain message string or an options object
 * ({ title, message, confirmLabel, cancelLabel }) and resolves to
 * true/false. The message renders with `white-space: pre-wrap`, so the
 * `\n\n`-joined warning strings inherited from confirm() carry over
 * verbatim. Pass `children` instead of `message` for rich JSX bodies.
 *
 * Unlike rendering <ConfirmModal/> directly, the dialog closes the moment
 * the choice is made (the host then runs its async action), matching the
 * imperative shape of the confirm() calls it replaces. For an action that
 * must keep the dialog up with a "working…" state, render <ConfirmModal/>
 * directly with `busy` instead.
 */
export function useConfirm() {
  const [opts, setOpts] = React.useState(null);
  const resolverRef = React.useRef(null);

  const confirm = React.useCallback((arg) => {
    return new Promise((resolve) => {
      resolverRef.current = resolve;
      setOpts(typeof arg === 'string' ? { message: arg } : (arg || {}));
    });
  }, []);

  // Resolve once and tear down. Idempotent: a second call (e.g. an Enter
  // keypress racing a click) finds no pending resolver and no-ops.
  const settle = React.useCallback((result) => {
    const resolve = resolverRef.current;
    resolverRef.current = null;
    setOpts(null);
    if (resolve) resolve(result);
  }, []);

  let confirmModal = null;
  if (opts) {
    const { message, children, ...rest } = opts;
    confirmModal = (
      <ConfirmModal
        {...rest}
        onConfirm={() => settle(true)}
        onCancel={() => settle(false)}
      >
        {children != null
          ? children
          : <div className="confirm-modal-message">{message}</div>}
      </ConfirmModal>
    );
  }

  return [confirm, confirmModal];
}
