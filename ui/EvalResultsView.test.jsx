import React from 'react';
import { describe, it, expect, afterEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import { EvalResultsView } from './EvalResultsView.jsx';

afterEach(cleanup);

const passing = {
  version: 1,
  spec_digest: 'sha256:abc',
  judge_model: 'anthropic/claude-sonnet-5',
  duration_ms: 12_345,
  cases: [
    {
      case_id: 'c1',
      output: 'out-1',
      output_truncated: false,
      case_score: 0.8,
      error: null,
      input: 'in-1',
      reference: 'ref-1',
      duration_ms: 1_234,
      dimensions: [
        { key: 'accuracy', rationale: 'looks right', score: 4, normalized: 0.8 },
        { key: 'tone', rationale: 'polite', score: 4, normalized: 0.8 },
      ],
    },
  ],
  aggregate: {
    per_dimension: { accuracy: { mean_normalized: 0.8 }, tone: { mean_normalized: 0.8 } },
    score: 0.8,
    pass_threshold: 0.75,
    pass: true,
    cases_total: 1,
    cases_errored: 0,
  },
  error: null,
};

describe('EvalResultsView', () => {
  it('renders a PASS verdict with score and per-dimension meters', () => {
    render(<EvalResultsView results={passing} status="passed" />);
    expect(screen.getByText('PASS')).toBeTruthy();
    expect(screen.getAllByText('80.0%').length).toBeGreaterThan(0);
    // Dimension labels appear (meter label + table header).
    expect(screen.getAllByText('accuracy').length).toBeGreaterThan(0);
    expect(screen.getByText('12.3s total')).toBeTruthy();
    expect(screen.getByText('1.23s')).toBeTruthy();
  });

  it('expands a case row to reveal input/candidate/rationale', () => {
    render(<EvalResultsView results={passing} status="passed" />);
    fireEvent.click(screen.getByText('c1'));
    expect(screen.getByText('in-1')).toBeTruthy();
    expect(screen.getByText('out-1')).toBeTruthy();
    expect(screen.getByText('looks right')).toBeTruthy();
  });

  it('can leave timing to a host-specific run view', () => {
    render(<EvalResultsView results={passing} status="passed" showTiming={false} />);
    expect(screen.queryByText('12.3s total')).toBeNull();
    expect(screen.queryByRole('columnheader', { name: 'time' })).toBeNull();
  });

  it('does not report missing historical timing as zero', () => {
    const historical = structuredClone(passing);
    delete historical.cases[0].duration_ms;
    render(<EvalResultsView results={historical} status="passed" />);
    expect(document.querySelector('.evalrep-case-time')?.textContent).toBe('—');
  });

  it('renders comparisons as exact agent and trajectory timing instead of a blended average', () => {
    const comparison = structuredClone(passing);
    comparison.duration_ms = 9_876;
    comparison.cases = [
      { ...comparison.cases[0], target: { key: 'prod', label: 'Prod' }, duration_ms: 1_234 },
      { ...comparison.cases[0], target: { key: 'test', label: 'Test' }, duration_ms: 2_345 },
    ];
    comparison.aggregate.cases_total = 2;
    comparison.comparison = { targets: [
      { target: { key: 'prod', label: 'Prod' }, aggregate: { ...comparison.aggregate, score: 0.8, cases_total: 1 }, duration_ms: 1_234 },
      { target: { key: 'test', label: 'Test' }, aggregate: { ...comparison.aggregate, score: 0.6, pass: false, cases_total: 1 }, duration_ms: 2_345 },
    ] };
    render(<EvalResultsView results={comparison} status="failed" />);
    expect(screen.getByText('COMPARE')).toBeTruthy();
    expect(screen.getByText('2 agents')).toBeTruthy();
    expect(screen.getByText('1.23s elapsed · 1 case · 0 errors')).toBeTruthy();
    expect(screen.getByText('2.35s elapsed · 1 case · 0 errors')).toBeTruthy();
    expect(screen.getByText('exact trajectories · no per-case averages')).toBeTruthy();
    expect(screen.getAllByText('Prod').length).toBeGreaterThan(0);
    expect(screen.queryByText('60.0%')).toBeNull();
    expect(screen.queryByText(/per-dimension mean/i)).toBeNull();
  });

  it('shows a running progress state before results arrive', () => {
    render(<EvalResultsView results={null} status="running" />);
    expect(screen.getByText('running evaluation…')).toBeTruthy();
  });

  it('surfaces a run-level fatal error', () => {
    render(
      <EvalResultsView
        results={{ version: 1, error: 'judge model not allowed by policy', aggregate: null, cases: [] }}
        status="error"
      />,
    );
    expect(screen.getByText('judge model not allowed by policy')).toBeTruthy();
  });
});
