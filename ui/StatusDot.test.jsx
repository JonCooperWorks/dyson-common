import React from 'react';
import { render, screen, cleanup } from '@testing-library/react';
import { afterEach, describe, expect, test } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { StatusDot } from './StatusDot.jsx';
import { SeverityTag, severityLevel } from './SeverityTag.jsx';

afterEach(cleanup);


describe('status and severity', () => {
  test('StatusDot always pairs its shape with a text label', () => {
    render(<StatusDot state="waiting"/>);
    expect(screen.getByText('Waiting on you')).toBeInTheDocument();
    const { container } = render(<StatusDot state="unreachable" label="Can't reach it"/>);
    expect(container.querySelector('.status-dot-unreachable')).toBeTruthy();
    expect(screen.getByText("Can't reach it")).toBeInTheDocument();
    const unknown = render(<StatusDot state="bogus"/>);
    expect(unknown.container.querySelector('.status-dot-idle')).toBeTruthy();
  });

  test('SeverityTag maps levels and labels them in sentence case', () => {
    render(<SeverityTag severity="critical"/>);
    expect(screen.getByText('High')).toBeInTheDocument();
    expect(severityLevel('warning')).toBe('medium');
    expect(severityLevel(undefined)).toBe('low');
  });
});
