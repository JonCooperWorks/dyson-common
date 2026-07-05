/* Shared security-report reader: renders a security-harness run's structured
 * report document as severity-grouped finding entities (cards + filters +
 * raw-JSON toggle). Used by dyson and dyson-swarm.
 *
 * The consumer passes `reportPath` and a `fallback` node. The document is
 * loaded by `load(reportPath)` — defaulting to the dyson workspace mind route
 * (GET /api/mind/file?path=...), which returns the file as a `content` string
 * inside a JSON envelope. Any failure (fetch error, non-OK, parse failure,
 * shape mismatch) renders `fallback` so a report is never less readable than
 * the plain-markdown render it replaces. Pass a custom `load` to source the
 * document from a different endpoint.
 *
 * Self-contained on purpose: no host classes, only `.secrep-*` (styled in
 * components.css) and the design tokens in tokens.css, so it renders
 * identically in both apps and in light/dark.
 */

import React, { useState, useEffect, useMemo } from 'react';
import { copyToClipboard } from './clipboard.js';

// Four-level severity scale the harness reports. `info`/`informational` and
// any unknown value fold into `low` — matching the backend SeverityRollup, and
// ensuring a finding never vanishes from the entity view over a mistyped
// severity.
const SEVERITY_LABELS = ['critical', 'high', 'medium', 'low'];
const SEVERITY_COLOR = {
  critical: '#cf2a2a',
  high: '#d97706',
  medium: '#ca8a04',
  low: '#6b7280',
};

function severityBucket(sev) {
  const s = String(sev || '').toLowerCase();
  if (s === 'critical' || s === 'high' || s === 'medium') return s;
  return 'low';
}

function matchesQuery(f, q) {
  if (!q) return true;
  const hay = [
    f.title, f.key, f.vulnerability_class, f.entry_point,
    f.sink_or_decision, f.root_cause,
    ...(Array.isArray(f.affected_paths) ? f.affected_paths : []),
  ].join('\n').toLowerCase();
  return hay.includes(q);
}

// Default loader: the dyson workspace mind route. `content` is the raw file
// STRING; torn writes (the workspace save is not atomic) surface here as a
// parse throw, which the caller turns into the markdown fallback.
async function loadFromMindRoute(reportPath) {
  const r = await fetch('/api/mind/file?path=' + encodeURIComponent(reportPath), { credentials: 'same-origin' });
  if (!r.ok) throw new Error(String(r.status));
  const payload = await r.json();
  const parsed = JSON.parse(payload.content);
  if (!parsed || !Array.isArray(parsed.findings)) throw new Error('bad doc');
  return parsed;
}

function Chevron() {
  return (
    <svg width={10} height={10} viewBox="0 0 16 16" fill="none" stroke="currentColor"
         strokeWidth={1.5} strokeLinecap="round" strokeLinejoin="round">
      <path d="m5 4 4 4-4 4"/>
    </svg>
  );
}

export function SecurityReportView({ reportPath, fallback, load = loadFromMindRoute }) {
  const [doc, setDoc] = useState(null);
  const [unavailable, setUnavailable] = useState(false);
  const [view, setView] = useState('cards');
  const [sevFilter, setSevFilter] = useState('');
  const [classFilter, setClassFilter] = useState('');
  const [flagFilter, setFlagFilter] = useState('all');
  const [query, setQuery] = useState('');
  const [expanded, setExpanded] = useState(() => new Set());
  const [copiedReport, setCopiedReport] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setDoc(null);
    setUnavailable(false);
    setExpanded(new Set());
    Promise.resolve()
      .then(() => load(reportPath))
      .then(parsed => {
        if (!parsed || !Array.isArray(parsed.findings)) throw new Error('bad doc');
        if (!cancelled) setDoc(parsed);
      })
      .catch(() => { if (!cancelled) setUnavailable(true); });
    return () => { cancelled = true; };
  }, [reportPath, load]);

  const classes = useMemo(() => {
    if (!doc) return [];
    const set = new Set(doc.findings.map(f => f.vulnerability_class).filter(Boolean));
    return [...set].sort();
  }, [doc]);

  if (unavailable || !doc) return fallback;

  const q = query.trim().toLowerCase();
  const visible = doc.findings
    .map((f, idx) => ({ f, idx }))
    .filter(({ f }) => !sevFilter || severityBucket(f.severity) === sevFilter)
    .filter(({ f }) => !classFilter || f.vulnerability_class === classFilter)
    .filter(({ f }) => flagFilter === 'all' || (flagFilter === 'recurring') === !!f.recurring)
    .filter(({ f }) => matchesQuery(f, q));
  const groups = SEVERITY_LABELS
    .map(sev => ({ sev, items: visible.filter(({ f }) => severityBucket(f.severity) === sev) }))
    .filter(g => g.items.length > 0);

  const toggle = (idx) => setExpanded(prev => {
    const next = new Set(prev);
    if (next.has(idx)) next.delete(idx); else next.add(idx);
    return next;
  });

  const copyReport = async () => {
    if (await copyToClipboard(JSON.stringify(doc, null, 2))) {
      setCopiedReport(true);
      setTimeout(() => setCopiedReport(false), 1200);
    }
  };

  const summary = doc.summary || {};
  return (
    <div className="secrep">
      <div className="secrep-inner">
        <div className="secrep-summary">
          <span className="secrep-summary-count">
            {doc.findings.length} {doc.findings.length === 1 ? 'finding' : 'findings'}
          </span>
          {SEVERITY_LABELS.map(sev => {
            const n = summary[sev] || 0;
            if (!n) return null;
            return (
              <span key={sev} className="secrep-sev">
                <span className="secrep-dot" style={{background: SEVERITY_COLOR[sev]}}/>
                <span className="secrep-sev-n">{n}</span>
                <span className="secrep-sev-label">{sev}</span>
              </span>
            );
          })}
          <span>{summary.new || 0} new · {summary.recurring || 0} recurring</span>
          <span className="secrep-mono secrep-summary-path" title={doc.target && doc.target.repo_path}>
            {doc.target && doc.target.repo_path}
          </span>
          {doc.model && doc.model.model && <span className="secrep-mono">{doc.model.model}</span>}
        </div>

        <div className="secrep-controls">
          {view === 'cards' && (
            <>
              <select className="secrep-select" value={sevFilter} onChange={e => setSevFilter(e.target.value)}>
                <option value="">all severities</option>
                {SEVERITY_LABELS.map(s => <option key={s} value={s}>{s}</option>)}
              </select>
              {classes.length > 0 && (
                <select className="secrep-select" value={classFilter} onChange={e => setClassFilter(e.target.value)}>
                  <option value="">all classes</option>
                  {classes.map(c => <option key={c} value={c}>{c}</option>)}
                </select>
              )}
              <select className="secrep-select" value={flagFilter} onChange={e => setFlagFilter(e.target.value)}>
                <option value="all">new + recurring</option>
                <option value="new">new</option>
                <option value="recurring">recurring</option>
              </select>
              <input className="secrep-search" type="search" placeholder="search"
                     value={query} onChange={e => setQuery(e.target.value)}/>
            </>
          )}
          <button className="secrep-btn" onClick={copyReport}>
            {copiedReport ? 'copied' : 'copy report'}
          </button>
          <span className="secrep-spacer"/>
          <button className="secrep-btn secrep-toggle" data-on={view === 'cards'}
                  onClick={() => setView('cards')}>rendered</button>
          <button className="secrep-btn secrep-toggle" data-on={view === 'json'}
                  onClick={() => setView('json')}>json</button>
        </div>

        {view === 'json' ? (
          <div className="secrep-json-wrap">
            <pre className="secrep-json">{JSON.stringify(doc, null, 2)}</pre>
          </div>
        ) : (
          <>
            {visible.length === 0 && (
              <div className="secrep-empty">
                {doc.findings.length === 0 ? 'No confirmed findings.' : 'No findings match.'}
              </div>
            )}
            {groups.map(({ sev, items }) => (
              <div key={sev} className="secrep-group">
                <div className="secrep-eyebrow">
                  <span className="secrep-dot" style={{background: SEVERITY_COLOR[sev]}}/>
                  {sev} · {items.length}
                </div>
                {items.map(({ f, idx }) => (
                  <FindingCard key={idx} finding={f} sev={severityBucket(f.severity)}
                               open={expanded.has(idx)} onToggle={() => toggle(idx)}/>
                ))}
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}

function FindingCard({ finding: f, sev, open, onToggle }) {
  const [copied, setCopied] = useState(false);
  const copyFinding = async (event) => {
    event.stopPropagation();
    if (await copyToClipboard(JSON.stringify(f, null, 2))) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    }
  };

  return (
    <div className="secrep-card">
      <div className="secrep-card-head" onClick={onToggle}>
        <span className="secrep-caret" style={{transform: open ? 'rotate(90deg)' : 'none'}}>
          <Chevron/>
        </span>
        <span className="secrep-dot" style={{background: SEVERITY_COLOR[sev]}}/>
        <span className="secrep-card-title">{f.title || f.run_finding_id || f.id}</span>
        {f.key && <span className="secrep-chip">{f.key}</span>}
        {f.recurring && <span className="secrep-chip">recurring x{f.occurrences}</span>}
        <button className="secrep-btn secrep-copy-finding" type="button" onClick={copyFinding}>
          {copied ? 'copied' : 'copy'}
        </button>
      </div>
      {open && (
        <div className="secrep-card-body">
          {row('class', f.vulnerability_class && <span className="secrep-chip">{f.vulnerability_class}</span>)}
          {row('boundary', text(f.trust_boundary))}
          {row('flow', (f.entry_point || f.sink_or_decision) && (
            <span className="secrep-mono">{f.entry_point}{f.entry_point && f.sink_or_decision ? ' → ' : ''}{f.sink_or_decision}</span>
          ))}
          {row('root cause', text(f.root_cause))}
          {row('reachability', text(f.reachability))}
          {row('impact', text(f.tenant_or_instance_impact))}
          {row('rationale', text(f.severity_rationale))}
          {row('fix', text(f.fix_recommendation))}
          {row('paths', Array.isArray(f.affected_paths) && f.affected_paths.length > 0 && (
            <span className="secrep-paths">
              {f.affected_paths.map((p, i) => <span key={i} className="secrep-chip">{p}</span>)}
            </span>
          ))}
          {row('evidence', Array.isArray(f.evidence) && f.evidence.length > 0 && (
            <pre className="secrep-pre secrep-pre-wrap">{f.evidence.join('\n')}</pre>
          ))}
          {row('patch', !!(f.suggested_patch && f.suggested_patch.trim()) && (
            <pre className="secrep-pre">{f.suggested_patch}</pre>
          ))}
        </div>
      )}
    </div>
  );
}

function text(v) {
  return v && String(v).trim() ? <span>{v}</span> : null;
}

function row(label, value) {
  if (!value) return null;
  return (
    <React.Fragment key={label}>
      <span className="secrep-label">{label}</span>
      <span className="secrep-value">{value}</span>
    </React.Fragment>
  );
}
