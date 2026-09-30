import { afterEach, describe, expect, test, vi } from 'vitest';
import React from 'react';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { Pager } from './Pager.jsx';

afterEach(cleanup);

describe('<Pager>', () => {
  test('names the range and moves page by page, clamped to the last', () => {
    const onPage = vi.fn();
    render(<Pager page={2} pages={4} from={50} shown={50} total={180} size={50} onPage={onPage} />);
    expect(screen.getByText(/51–100 of 180 rows/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Next page' }));
    fireEvent.click(screen.getByRole('button', { name: 'Last page' }));
    fireEvent.click(screen.getByRole('button', { name: 'First page' }));
    const input = screen.getByRole('spinbutton', { name: 'Page number' });
    fireEvent.change(input, { target: { value: '99' } });
    fireEvent.submit(input.closest('form'));
    expect(onPage.mock.calls.map(c => c[0])).toEqual([3, 4, 1, 4]);
  });

  test('an unknown end offers next only while a full page came back', () => {
    const { rerender } = render(<Pager page={1} pages={null} from={0} shown={25} total={25} size={25} onPage={() => {}} />);
    expect(screen.getByRole('button', { name: 'Next page' })).toBeEnabled();
    expect(screen.queryByRole('button', { name: 'Last page' })).toBeNull();
    rerender(<Pager page={2} pages={null} from={25} shown={10} total={35} size={25} onPage={() => {}} />);
    expect(screen.getByRole('button', { name: 'Next page' })).toBeDisabled();
  });

  test('rows per page is a segmented choice', () => {
    const onSize = vi.fn();
    render(<Pager page={1} pages={1} from={0} shown={3} total={3} size={50} onPage={() => {}} onSize={onSize} noun={['file', 'files']} />);
    expect(screen.getByText(/1–3 of 3 files/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '50' })).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(screen.getByRole('button', { name: '100' }));
    expect(onSize).toHaveBeenCalledWith(100);
  });
});
