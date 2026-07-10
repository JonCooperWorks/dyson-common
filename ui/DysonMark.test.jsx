// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, act, cleanup } from '@testing-library/react';
import React from 'react';
import { DysonMark, ComputerMark, setBrandMarkVariant } from './DysonMark.jsx';
import { MARK_VARIANTS, MARK_VARIANT_NAMES, MARK_VARIANT_LABELS, DEFAULT_MARK_VARIANT } from './marks.js';

describe('mark variants', () => {
  it('every variant has shapes and a label', () => {
    expect(MARK_VARIANT_NAMES.length).toBeGreaterThanOrEqual(8);
    for (const name of MARK_VARIANT_NAMES) {
      expect(MARK_VARIANTS[name].length).toBeGreaterThan(0);
      expect(MARK_VARIANT_LABELS[name]).toBeTruthy();
    }
  });

  it('renders every variant without crashing', () => {
    for (const name of MARK_VARIANT_NAMES) {
      const { container, unmount } = render(<DysonMark variant={name}/>);
      expect(container.querySelectorAll('path, circle').length).toBeGreaterThan(0);
      unmount();
    }
  });

  it('follows the app-wide variant and updates live', () => {
    try {
      const { container } = render(<DysonMark/>);
      const before = container.innerHTML;
      act(() => setBrandMarkVariant('aperture'));
      expect(container.innerHTML).not.toEqual(before);
      act(() => setBrandMarkVariant('bogus-name'));
      expect(container.innerHTML).toEqual(before); // falls back to default
    } finally {
      act(() => setBrandMarkVariant(DEFAULT_MARK_VARIANT));
      cleanup();
    }
  });

  it('an explicit variant prop wins over the app-wide default', () => {
    try {
      act(() => setBrandMarkVariant('eclipse'));
      const pinned = render(<ComputerMark variant={DEFAULT_MARK_VARIANT}/>);
      const reference = render(<ComputerMark variant={DEFAULT_MARK_VARIANT}/>);
      expect(pinned.container.innerHTML).toEqual(reference.container.innerHTML);
    } finally {
      act(() => setBrandMarkVariant(DEFAULT_MARK_VARIANT));
      cleanup();
    }
  });
});
