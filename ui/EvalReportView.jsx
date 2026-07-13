/* Shared eval-report card: loads an eval run's document and renders it with the
 * shared EvalResultsView, so a run launched by an agent shows inline in the
 * Artefacts tab exactly as it does on the operator's Evals run page.
 *
 * The consumer passes `reportPath` (the run id) and a `fallback` node. The run
 * document is loaded by `load(reportPath)`; it may return either a run view
 * (`{ status, results, ... }`) or a bare §3.4 results document — both are
 * normalized here. While the run is pending/running the view polls so the card
 * transitions to its terminal render on its own. Any load failure renders
 * `fallback`, so an eval artefact is never less readable than the plain body it
 * replaces. Pass a custom `load` to source the document from a different
 * endpoint (dyson-swarm injects the swarm run-poll; dyson uses the mind route).
 *
 * Self-contained: only `.evalrep-*` (components.css) + design tokens, so it
 * renders identically in both apps and in light/dark.
 */

import React, { useState, useEffect, useRef, useCallback } from 'react';
import { EvalResultsView } from './EvalResultsView.jsx';

const TERMINAL = new Set(['passed', 'failed', 'error']);
const POLL_MS = 2500;

// Default loader: the dyson workspace mind route (same envelope security reports
// use). `content` is the raw file STRING.
async function loadFromMindRoute(reportPath) {
  const r = await fetch('/api/mind/file?path=' + encodeURIComponent(reportPath), { credentials: 'same-origin' });
  if (!r.ok) throw new Error(String(r.status));
  const payload = await r.json();
  return JSON.parse(payload.content);
}

/// Normalize whatever the loader returned into `{ status, results, history? }`.
function normalize(doc) {
  if (!doc || typeof doc !== 'object') throw new Error('bad doc');
  // A run view carries `results` (the §3.4 doc) + `status`; a bare results doc
  // carries `aggregate`/`cases` directly.
  const results = doc.results != null ? doc.results : (doc.aggregate || doc.cases ? doc : null);
  const status = doc.status
    || (results && results.error ? 'error' : null)
    || (results && results.aggregate ? (results.aggregate.pass ? 'passed' : 'failed') : 'pending');
  return { status, results, history: doc.history };
}

export function EvalReportView({ reportPath, fallback, load = loadFromMindRoute, history }) {
  const [state, setState] = useState(null); // { status, results, history }
  const [unavailable, setUnavailable] = useState(false);
  const timer = useRef(null);

  const clearTimer = () => {
    if (timer.current) { clearTimeout(timer.current); timer.current = null; }
  };

  const fetchOnce = useCallback(async (cancelledRef) => {
    try {
      const doc = await Promise.resolve(load(reportPath));
      const norm = normalize(doc);
      if (cancelledRef.cancelled) return;
      setState(norm);
      // Keep polling until terminal.
      if (!TERMINAL.has(norm.status)) {
        clearTimer();
        timer.current = setTimeout(() => fetchOnce(cancelledRef), POLL_MS);
      }
    } catch {
      if (!cancelledRef.cancelled) setUnavailable(true);
    }
  }, [load, reportPath]);

  useEffect(() => {
    const cancelledRef = { cancelled: false };
    setState(null);
    setUnavailable(false);
    fetchOnce(cancelledRef);
    return () => {
      cancelledRef.cancelled = true;
      clearTimer();
    };
  }, [fetchOnce]);

  if (unavailable) return fallback;
  if (!state) {
    return (
      <div className="evalrep">
        <div className="evalrep-progress evalrep-progress-pending">
          <span className="evalrep-spinner" aria-hidden="true" />
          <span className="evalrep-progress-label">loading eval…</span>
        </div>
      </div>
    );
  }
  return (
    <EvalResultsView
      results={state.results}
      status={state.status}
      history={history || state.history}
    />
  );
}
