import React from 'react';
import { copyText } from './clipboard.js';

export function PageHeader({ eyebrow, title, description, actions, children, className = '' }) {
  return <header className={`gateway-page-header ${className}`.trim()}><div className="gateway-page-heading">{eyebrow ? <div className="gateway-page-eyebrow">{eyebrow}</div> : null}<h1>{title}</h1>{description ? <p>{description}</p> : null}</div>{actions ? <div className="gateway-page-actions">{actions}</div> : null}{children}</header>;
}

export function MetricGrid({ children, className = '' }) {
  return <div className={`gateway-metric-grid ${className}`.trim()}>{children}</div>;
}

export function MetricCard({ label, value, detail, tone = 'neutral', trend, className = '' }) {
  return <article className={`gateway-metric gateway-tone-${tone} ${className}`.trim()}><div className="gateway-metric-label">{label}</div><div className="gateway-metric-row"><strong>{value}</strong>{trend ? <span className="gateway-metric-trend">{trend}</span> : null}</div>{detail ? <div className="gateway-metric-detail">{detail}</div> : null}</article>;
}

export function StatusBadge({ children, tone = 'neutral', pulse = false, className = '' }) {
  return <span className={`gateway-status gateway-tone-${tone} ${pulse ? 'gateway-status-pulse' : ''} ${className}`.trim()}><span className="gateway-status-dot" aria-hidden="true" />{children}</span>;
}

export function Toolbar({ children, className = '', 'aria-label': ariaLabel = 'Filters' }) {
  return <div className={`gateway-toolbar ${className}`.trim()} role="search" aria-label={ariaLabel}>{children}</div>;
}

export function DataTable({ columns, rows, rowKey = 'id', empty = null, onRowClick, className = '' }) {
  if (!rows.length && empty) return empty;
  return <div className={`gateway-table-wrap ${className}`.trim()}><table className="gateway-table"><thead><tr>{columns.map(column => <th key={column.key}>{column.label}</th>)}</tr></thead><tbody>{rows.map((row, index) => { const key = typeof rowKey === 'function' ? rowKey(row, index) : row[rowKey] ?? index; return <tr key={key} onClick={onRowClick ? () => onRowClick(row) : undefined} className={onRowClick ? 'gateway-row-action' : ''}>{columns.map(column => <td key={column.key} data-label={column.label}>{column.render ? column.render(row[column.key], row) : row[column.key]}</td>)}</tr>; })}</tbody></table></div>;
}

export function DetailList({ items, className = '' }) {
  return <dl className={`gateway-detail-list ${className}`.trim()}>{items.map(item => <div className="gateway-detail-row" key={item.label}><dt>{item.label}</dt><dd>{item.value ?? '—'}</dd></div>)}</dl>;
}

// Copy says what happened: "Copied", or — where the browser refuses the
// clipboard (plain-http hosts, denied permission) — the text is selected
// for a manual ⌘C, or "Copy failed".
export function JsonCodeBlock({ value, label = 'JSON', copyLabel = 'Copy', className = '' }) {
  const [state, setState] = React.useState('');
  const code = React.useRef(null);
  const text = typeof value === 'string' ? value : JSON.stringify(value, null, 2);
  async function copy() { const result = await copyText(text, code.current); setState(result); if (result !== 'selected') window.setTimeout(() => setState(''), 1500); }
  const shown = state === 'copied' ? 'Copied' : state === 'selected' ? 'Selected · press ⌘C' : state === 'failed' ? 'Copy failed' : copyLabel;
  return <section className={`gateway-code ${className}`.trim()}><div className="gateway-code-head"><span>{label}</span><button type="button" onClick={copy}>{shown}</button></div><pre><code ref={code}>{text}</code></pre></section>;
}
