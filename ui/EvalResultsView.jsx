/* Shared eval-results renderer — the one results vocabulary the operator's
 * Evals run page (dyson-swarm SPA) and the in-agent EvalReport card both
 * consume, so the page and the card read as the same product.
 *
 * Pure + presentational: give it a parsed §3.4 results document (`results`) and
 * the run's `status`; it renders the pass/fail verdict, per-dimension score
 * meters, an optional run-history trend, and an expandable per-case table. No
 * fetching, no host classes — only `.evalrep-*` (styled in components.css) and
 * the design tokens in tokens.css, so it renders identically in both apps and
 * in light/dark.
 */

import React, { useState } from 'react';

const TERMINAL = new Set(['passed', 'failed', 'error']);

function pct(x) {
  if (!Number.isFinite(x)) return '—';
  return `${(x * 100).toFixed(1)}%`;
}

function statusTone(status) {
  if (status === 'passed') return 'ok';
  if (status === 'failed') return 'fail';
  if (status === 'error') return 'error';
  return 'pending';
}

function Chevron() {
  return (
    <svg width={10} height={10} viewBox="0 0 16 16" fill="none" stroke="currentColor"
         strokeWidth={1.5} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="m5 4 4 4-4 4" />
    </svg>
  );
}

/// Normalized 0..1 meter with an aligned label + value. `tone` selects the fill.
function Meter({ label, value, tone = 'accent', title }) {
  const clamped = Math.max(0, Math.min(1, Number(value) || 0));
  return (
    <div className="evalrep-meter" title={title}>
      <span className="evalrep-meter-label">{label}</span>
      <span className="evalrep-meter-track">
        <span
          className={`evalrep-meter-fill evalrep-fill-${tone}`}
          style={{ width: `${clamped * 100}%` }}
        />
      </span>
      <span className="evalrep-meter-value">{pct(clamped)}</span>
    </div>
  );
}

/// Compact inline sparkline of aggregate score across successive runs, with the
/// pass threshold drawn as a reference line. Oldest → newest, left → right.
function TrendSparkline({ history, threshold }) {
  const points = (history || [])
    .map((r) => ({
      score: Number(r?.score),
      passed: r?.status === 'passed',
    }))
    .filter((p) => Number.isFinite(p.score));
  if (points.length < 2) return null;

  const W = 240;
  const H = 44;
  const pad = 4;
  const n = points.length;
  const x = (i) => pad + (i * (W - 2 * pad)) / (n - 1);
  const y = (s) => H - pad - s * (H - 2 * pad);
  const path = points.map((p, i) => `${i === 0 ? 'M' : 'L'}${x(i).toFixed(1)},${y(p.score).toFixed(1)}`).join(' ');
  const thr = Number.isFinite(threshold) ? y(threshold) : null;

  return (
    <div className="evalrep-trend">
      <span className="evalrep-trend-label">score trend · {n} runs</span>
      <svg className="evalrep-trend-svg" viewBox={`0 0 ${W} ${H}`} width={W} height={H}
           preserveAspectRatio="none" role="img" aria-label="score trend across runs">
        {thr != null && (
          <line x1={0} x2={W} y1={thr} y2={thr} className="evalrep-trend-threshold"
                strokeDasharray="3 3" />
        )}
        <path d={path} className="evalrep-trend-line" fill="none" />
        {points.map((p, i) => (
          <circle key={i} cx={x(i)} cy={y(p.score)} r={2.6}
                  className={`evalrep-trend-dot evalrep-dot-${p.passed ? 'ok' : 'fail'}`} />
        ))}
      </svg>
    </div>
  );
}

function CaseRow({ c, dimKeys, open, onToggle }) {
  const errored = !!c.error;
  const byKey = new Map((c.dimensions || []).map((d) => [d.key, d]));
  return (
    <>
      <tr className={`evalrep-case-row${errored ? ' evalrep-case-errored' : ''}`} onClick={onToggle}>
        <td className="evalrep-case-toggle">
          <span className="evalrep-caret" style={{ transform: open ? 'rotate(90deg)' : 'none' }}>
            <Chevron />
          </span>
        </td>
        <td className="evalrep-case-id" title={c.case_id}>{c.case_id}</td>
        {dimKeys.map((k) => {
          const d = byKey.get(k);
          return (
            <td key={k} className="evalrep-case-dim">
              {d ? (
                <span className="evalrep-dimcell" title={`raw ${d.score}`}>
                  <span className="evalrep-dimbar">
                    <span className="evalrep-dimbar-fill" style={{ width: `${Math.max(0, Math.min(1, d.normalized)) * 100}%` }} />
                  </span>
                  <span className="evalrep-dimcell-val">{pct(d.normalized)}</span>
                </span>
              ) : <span className="evalrep-muted">—</span>}
            </td>
          );
        })}
        <td className="evalrep-case-score">{errored ? <span className="evalrep-muted">—</span> : pct(c.case_score)}</td>
        <td className="evalrep-case-state">
          {errored
            ? <span className="evalrep-chip evalrep-chip-error">error</span>
            : <span className="evalrep-chip evalrep-chip-ok">scored</span>}
        </td>
      </tr>
      {open && (
        <tr className="evalrep-case-detail-row">
          <td colSpan={dimKeys.length + 4}>
            <div className="evalrep-case-detail">
              {errored && <div className="evalrep-error-note">{c.error}</div>}
              <DetailBlock label="input" value={c.input} />
              <DetailBlock label="reference" value={c.reference} />
              <DetailBlock label="candidate output" value={c.output} truncated={c.output_truncated} />
              {(c.dimensions || []).length > 0 && (
                <div className="evalrep-rationales">
                  {c.dimensions.map((d) => (
                    <div key={d.key} className="evalrep-rationale">
                      <div className="evalrep-rationale-head">
                        <span className="evalrep-rationale-key">{d.key}</span>
                        <span className="evalrep-rationale-score">{d.score} · {pct(d.normalized)}</span>
                      </div>
                      <div className="evalrep-rationale-text">{d.rationale}</div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </td>
        </tr>
      )}
    </>
  );
}

function DetailBlock({ label, value, truncated }) {
  if (value == null || value === '') return null;
  return (
    <div className="evalrep-detail-block">
      <div className="evalrep-detail-label">
        {label}{truncated ? <span className="evalrep-muted"> · truncated</span> : null}
      </div>
      <pre className="evalrep-pre">{value}</pre>
    </div>
  );
}

export function EvalResultsView({ results, status, history, title }) {
  const [expanded, setExpanded] = useState(() => new Set());
  const isTerminal = TERMINAL.has(status) || (results && (results.error || results.aggregate));

  // Pending / running — no results yet. Keep a calm, alive progress state so the
  // page and card transition smoothly to the terminal render.
  if (!results || !results.aggregate) {
    if (isTerminal && results && results.error) {
      return (
        <div className="evalrep">
          {title ? <div className="evalrep-title">{title}</div> : null}
          <div className="evalrep-fatal">{results.error}</div>
        </div>
      );
    }
    const s = status || 'pending';
    return (
      <div className="evalrep">
        {title ? <div className="evalrep-title">{title}</div> : null}
        <div className={`evalrep-progress evalrep-progress-${statusTone(s)}`}>
          <span className="evalrep-spinner" aria-hidden="true" />
          <span className="evalrep-progress-label">{s === 'running' ? 'running evaluation…' : 'queued…'}</span>
        </div>
      </div>
    );
  }

  const agg = results.aggregate;
  const perDim = agg.per_dimension || {};
  const dimKeys = Object.keys(perDim);
  const runStatus = status || (agg.pass ? 'passed' : 'failed');
  const fatal = results.error;

  const toggle = (idx) => setExpanded((prev) => {
    const next = new Set(prev);
    if (next.has(idx)) next.delete(idx); else next.add(idx);
    return next;
  });

  return (
    <div className="evalrep">
      {title ? <div className="evalrep-title">{title}</div> : null}

      {fatal ? <div className="evalrep-fatal">{fatal}</div> : null}

      {/* Verdict — aggregate score vs threshold, pass/fail. */}
      <div className={`evalrep-verdict evalrep-verdict-${statusTone(runStatus)}`}>
        <div className="evalrep-verdict-main">
          <span className="evalrep-verdict-badge">
            {runStatus === 'passed' ? 'PASS' : runStatus === 'failed' ? 'FAIL' : 'ERROR'}
          </span>
          <span className="evalrep-verdict-score">{pct(agg.score)}</span>
          <span className="evalrep-verdict-threshold">vs {pct(agg.pass_threshold)} threshold</span>
        </div>
        <div className="evalrep-verdict-gauge">
          <span className="evalrep-gauge-track">
            <span className={`evalrep-gauge-fill evalrep-fill-${statusTone(runStatus)}`}
                  style={{ width: `${Math.max(0, Math.min(1, agg.score)) * 100}%` }} />
            <span className="evalrep-gauge-threshold"
                  style={{ left: `${Math.max(0, Math.min(1, agg.pass_threshold)) * 100}%` }} />
          </span>
        </div>
        <div className="evalrep-verdict-meta">
          <span>{agg.cases_total} case{agg.cases_total === 1 ? '' : 's'}</span>
          {agg.cases_errored > 0 ? <span className="evalrep-meta-error">{agg.cases_errored} errored</span> : null}
          {results.judge_model ? <span className="evalrep-mono">judge · {results.judge_model}</span> : null}
        </div>
      </div>

      <TrendSparkline history={history} threshold={agg.pass_threshold} />

      {/* Per-dimension mean meters. */}
      {dimKeys.length > 0 && (
        <div className="evalrep-dims">
          <div className="evalrep-section-label">per-dimension mean</div>
          {dimKeys.map((k) => (
            <Meter key={k} label={k} value={perDim[k]?.mean_normalized} tone="accent"
                   title={`${k}: ${pct(perDim[k]?.mean_normalized)}`} />
          ))}
        </div>
      )}

      {/* Case table with expandable detail. */}
      <div className="evalrep-cases">
        <div className="evalrep-section-label">cases</div>
        <div className="evalrep-table-wrap">
          <table className="evalrep-table">
            <thead>
              <tr>
                <th aria-label="expand" />
                <th>case</th>
                {dimKeys.map((k) => <th key={k} className="evalrep-th-dim">{k}</th>)}
                <th>score</th>
                <th>state</th>
              </tr>
            </thead>
            <tbody>
              {(results.cases || []).map((c, i) => (
                <CaseRow key={c.case_id || i} c={c} dimKeys={dimKeys}
                         open={expanded.has(i)} onToggle={() => toggle(i)} />
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
