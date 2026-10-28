// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { render, cleanup } from '@testing-library/react';
import React from 'react';
import { GlitchMark } from './GlitchMark.jsx';

afterEach(cleanup);

describe('GlitchMark', () => {
  it('tears the orbits at display sizes', () => {
    const { container } = render(<GlitchMark size={48}/>);
    expect(container.querySelectorAll('clipPath')).toHaveLength(3);
    expect(container.innerHTML).toContain('#ff3b4e');
  });

  it('drops the tear at 24 px and below, where it turns to noise', () => {
    const { container } = render(<GlitchMark size={24}/>);
    expect(container.querySelector('clipPath')).toBeNull();
    expect(container.querySelectorAll('path, circle').length).toBeGreaterThan(0);
  });

  it('keeps clip ids unique per mark', () => {
    const { container } = render(<><GlitchMark size={48}/><GlitchMark size={48}/></>);
    const ids = [...container.querySelectorAll('clipPath')].map(c => c.id);
    expect(new Set(ids).size).toBe(6);
  });
});
