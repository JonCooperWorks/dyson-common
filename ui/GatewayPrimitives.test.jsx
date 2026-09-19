import { afterEach, expect, test, vi } from 'vitest';
import React from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { DataTable, JsonCodeBlock, MetricCard, StatusBadge } from './GatewayPrimitives.jsx';

afterEach(cleanup);

test('renders gateway metrics and status semantics', () => {
  render(<><MetricCard label="Requests" value="42" detail="last hour" tone="ok"/><StatusBadge tone="danger">blocked</StatusBadge></>);
  expect(screen.getByText('42')).toBeInTheDocument();
  expect(screen.getByText('blocked')).toHaveClass('gateway-tone-danger');
});

test('data table renders custom cells and dispatches selected row', () => {
  const selected = vi.fn();
  render(<DataTable columns={[{ key: 'status', label: 'Status', render: value => value.toUpperCase() }]} rows={[{ id: 'r1', status: 'ok' }]} onRowClick={selected}/>);
  fireEvent.click(screen.getByText('OK'));
  expect(selected).toHaveBeenCalledWith({ id: 'r1', status: 'ok' });
});

test('json block formats structured values', () => {
  render(<JsonCodeBlock value={{ model: 'openai/gpt-5' }}/>);
  expect(screen.getByText(/"model": "openai\/gpt-5"/)).toBeInTheDocument();
});
