import React from 'react';
import { describe, it, expect, afterEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import { EvalResultsView } from './EvalResultsView.jsx';

afterEach(cleanup);

const passing = {
  version: 1,
  spec_digest: 'sha256:abc',
  judge_model: 'anthropic/claude-sonnet-5',
  cases: [
    {
      case_id: 'c1',
      output: 'out-1',
      output_truncated: false,
      case_score: 0.8,
      error: null,
      input: 'in-1',
      reference: 'ref-1',
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
  });

  it('expands a case row to reveal input/candidate/rationale', () => {
    render(<EvalResultsView results={passing} status="passed" />);
    fireEvent.click(screen.getByText('c1'));
    expect(screen.getByText('in-1')).toBeTruthy();
    expect(screen.getByText('out-1')).toBeTruthy();
    expect(screen.getByText('looks right')).toBeTruthy();
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
