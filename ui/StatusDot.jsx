import React from 'react';

// Spec 4.4: an 8 px dot whose shape, not only its colour, says the state, and
// always next to a text label (the label is the accessible name here too).
const STATES = {
  working: 'Working',
  waiting: 'Waiting on you',
  idle: 'Idle',
  failed: 'Failed',
  unreachable: 'Unreachable',
  stopped: 'Stopped',
};

export const STATUS_DOT_STATES = Object.keys(STATES);

/// `state`: working | waiting | idle | failed | unreachable | stopped.
/// `label` overrides the default text; `hideLabel` keeps it for screen
/// readers only (use only where a visible label sits beside the dot).
export function StatusDot({ state = 'idle', label, hideLabel = false, className = '' }) {
  const s = STATES[state] ? state : 'idle';
  const text = label || STATES[s];
  return (
    <span className={`status-dot-wrap${className ? ` ${className}` : ''}`}>
      <span className={`status-dot status-dot-${s}`} aria-hidden="true"/>
      <span className={hideLabel ? 'sr-only' : 'status-dot-label'}>{text}</span>
    </span>
  );
}
