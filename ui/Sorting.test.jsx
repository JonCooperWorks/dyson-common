import { afterEach, describe, expect, test, vi } from 'vitest';
import React, { useState } from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { SortPill, SortTh, useSorter } from './Sorting.jsx';

afterEach(cleanup);

const COLUMNS = { when: ['Time', 'time'], name: ['Name', 'text'], ttfb: ['Time to first byte', 'num'] };

function Table({ initial = '', onChange = () => {} }) {
  const [value, setValue] = useState(initial);
  const sorter = useSorter(COLUMNS, { value, onChange: v => { setValue(v); onChange(v); } });
  return <><table><thead><tr>
    <SortTh sorter={sorter} keys={['when', 'ttfb']}>Time</SortTh>
    <SortTh sorter={sorter} keys={['name']}>Name</SortTh>
  </tr></thead></table><SortPill sorter={sorter} /><span data-testid="value">{value}</span></>;
}

describe('sortable heads', () => {
  test('a click cycles natural direction, reversed, then unsorted', () => {
    const onChange = vi.fn();
    render(<Table onChange={onChange} />);
    const name = screen.getByRole('button', { name: /Name/ });
    fireEvent.click(name);
    fireEvent.click(name);
    fireEvent.click(name);
    expect(onChange.mock.calls.map(c => c[0])).toEqual(['name', '-name', '']);
  });

  test('times sort newest first and mark the head', () => {
    render(<Table />);
    fireEvent.click(screen.getByRole('button', { name: /^Time/ }));
    expect(screen.getByTestId('value')).toHaveTextContent('-when');
    expect(screen.getAllByRole('columnheader')[0]).toHaveAttribute('aria-sort', 'descending');
    expect(screen.getByRole('status')).toHaveTextContent('Sorted by Time · newest first');
  });

  test('a second key is picked from the menu and named in the head', () => {
    render(<Table />);
    fireEvent.click(screen.getByRole('button', { name: 'More ways to sort Time' }));
    fireEvent.click(screen.getAllByRole('menuitemradio', { name: /lowest first/ })[0]);
    expect(screen.getByTestId('value')).toHaveTextContent('ttfb');
    expect(screen.getAllByRole('columnheader')[0]).toHaveTextContent('Time to first byte');
  });

  test('the pill goes back to the default order; unknown keys are ignored', () => {
    render(<Table initial="-name" />);
    fireEvent.click(screen.getByRole('button', { name: /Stop sorting/ }));
    expect(screen.getByTestId('value')).toHaveTextContent('');
    cleanup();
    render(<Table initial="bogus" />);
    expect(screen.queryByRole('status')).toBeNull();
  });
});
