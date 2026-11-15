import React from 'react';

// Spec 4.3: the only place the three severity colours appear together.
// Each level has a shape cue (filled square, filled triangle, hollow circle)
// so it never relies on colour alone.
const LABELS = { high: 'High', medium: 'Medium', low: 'Low' };

export function severityLevel(value) {
  const v = String(value || '').toLowerCase();
  if (v === 'critical' || v === 'high' || v === 'error') return 'high';
  if (v === 'medium' || v === 'moderate' || v === 'warning' || v === 'warn') return 'medium';
  return 'low';
}

export function SeverityTag({ severity, label, className = '' }) {
  const level = severityLevel(severity);
  return (
    <span className={`severity-tag severity-${level}${className ? ` ${className}` : ''}`}>
      <span className="severity-shape" aria-hidden="true"/>
      {label || LABELS[level]}
    </span>
  );
}
