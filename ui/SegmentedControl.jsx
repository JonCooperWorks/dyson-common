import React from 'react';

/* A segmented control: two to six mutually exclusive choices shown side
 * by side. `options` are `[value, label, icon?]`, where `icon` is any
 * node (the app brings its own icon set). Each segment is a pressed /
 * not-pressed button, so the group reads correctly to a screen reader. */
export function SegmentedControl({ value, onChange, options, label, className = '' }) {
  return <div className={`dc-seg ${className}`.trim()} role="group" aria-label={label}>{options.map(([id, text, icon]) =>
    <button type="button" key={String(id)} aria-pressed={value === id} className={value === id ? 'active' : ''} onClick={() => onChange(id)}>{icon}{text}</button>)}</div>;
}
