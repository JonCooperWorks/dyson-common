/* Tests for <EmptyState>. */
import { describe, expect, test, afterEach } from 'vitest';
import React from 'react';
import { render, screen, cleanup } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';

import { EmptyState } from './EmptyState.jsx';

afterEach(cleanup);

describe('EmptyState', () => {
  test('renders the title and default glyph', () => {
    render(React.createElement(EmptyState, { title: 'nothing here' }));
    expect(screen.getByText('nothing here')).toBeInTheDocument();
    expect(document.querySelector('.ui-empty-glyph')).toHaveTextContent('∅');
  });

  test('renders a custom glyph, body, and actions', () => {
    render(
      React.createElement(
        EmptyState,
        { glyph: '★', title: 'empty', actions: React.createElement('button', {}, 'add') },
        'try adding one',
      ),
    );
    expect(document.querySelector('.ui-empty-glyph')).toHaveTextContent('★');
    expect(screen.getByText('try adding one')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'add' })).toBeInTheDocument();
  });
});
