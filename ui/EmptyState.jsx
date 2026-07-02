import React from 'react';

/**
 * Placeholder shown when a list or panel has no rows yet: a centred glyph,
 * a title, optional body copy, and optional action controls.
 */
export function EmptyState({ glyph = '∅', title, children, actions = null, className = '' }) {
  return (
    <div className={`ui-empty ${className}`.trim()}>
      <div className="ui-empty-glyph" aria-hidden="true">{glyph}</div>
      <div className="ui-empty-title">{title}</div>
      {children ? <div className="ui-empty-body muted small">{children}</div> : null}
      {actions ? <div className="ui-empty-actions">{actions}</div> : null}
    </div>
  );
}
