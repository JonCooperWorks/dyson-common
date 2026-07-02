/* Tests for <ConfirmModal> and the useConfirm() promise wrapper. */
import { describe, expect, test, afterEach, vi } from 'vitest';
import React from 'react';
import { render, screen, fireEvent, cleanup, act } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';

import { ConfirmModal, useConfirm } from './ConfirmModal.jsx';

afterEach(cleanup);

describe('ConfirmModal', () => {
  test('renders title, body, and labelled action buttons', () => {
    render(
      React.createElement(
        ConfirmModal,
        { title: 'delete X?', confirmLabel: 'delete', onConfirm: vi.fn(), onCancel: vi.fn() },
        'this cannot be undone',
      ),
    );
    expect(screen.getByRole('dialog')).toHaveAttribute('aria-label', 'delete X?');
    expect(screen.getByText('this cannot be undone')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'delete' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'cancel' })).toBeInTheDocument();
  });

  test('confirm / cancel buttons fire their handlers', () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    render(
      React.createElement(ConfirmModal, { title: 't', onConfirm, onCancel }, 'body'),
    );
    fireEvent.click(screen.getByRole('button', { name: 'confirm' }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'cancel' }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  test('busy disables the buttons and shows the working label', () => {
    render(
      React.createElement(
        ConfirmModal,
        { title: 't', busy: true, onConfirm: vi.fn(), onCancel: vi.fn() },
        'body',
      ),
    );
    expect(screen.getByRole('button', { name: 'working…' })).toBeDisabled();
  });
});

describe('useConfirm', () => {
  function Host() {
    const [confirm, confirmModal] = useConfirm();
    return (
      <>
        <button onClick={async () => { window.__result = await confirm({ title: 'sure?' }); }}>
          ask
        </button>
        {confirmModal}
      </>
    );
  }

  test('resolves true when confirmed and closes the dialog', async () => {
    render(<Host />);
    expect(screen.queryByRole('dialog')).toBeNull();

    await act(async () => { fireEvent.click(screen.getByText('ask')); });
    expect(screen.getByRole('dialog')).toBeInTheDocument();

    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'confirm' })); });
    expect(window.__result).toBe(true);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  test('resolves false when cancelled', async () => {
    render(<Host />);
    await act(async () => { fireEvent.click(screen.getByText('ask')); });
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'cancel' })); });
    expect(window.__result).toBe(false);
  });
});
