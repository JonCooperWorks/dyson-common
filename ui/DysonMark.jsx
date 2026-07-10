import React, { useSyncExternalStore } from 'react';
import { createStore } from './createStore.js';
import { MARK_VARIANTS, DEFAULT_MARK_VARIANT } from './marks.js';

// The Dyson brand mark: flat, single-colour glyphs in the spirit of the
// OpenAI / Anthropic marks, tinted the Dyson brand blue by default. The
// glyph geometry lives in ./marks.js as one of several selectable variants
// ('classic' is the original collector-panel ring). Which variant renders is
// a prop, falling back to an app-wide default that the host app sets from
// its runtime config via setBrandMarkVariant — every mounted mark re-renders
// live when it changes.

// Default brand blue — reads on both light and dark surfaces.
export const DYSON_BLUE = '#3b82f6';

const markStore = createStore({ variant: DEFAULT_MARK_VARIANT });

export function setBrandMarkVariant(variant) {
  const next = MARK_VARIANTS[variant] ? variant : DEFAULT_MARK_VARIANT;
  markStore.dispatch((s) => (s.variant === next ? s : { variant: next }));
}

export function useBrandMarkVariant() {
  return useSyncExternalStore(markStore.subscribe, () => markStore.getSnapshot().variant);
}

// The bare mark, origin-centred in the shared -112..112 design box, ready to
// drop into a viewBox or transform. Shape kinds are documented in marks.js.
export function Glyph({ color, variant }) {
  const shapes = MARK_VARIANTS[variant] ?? MARK_VARIANTS[DEFAULT_MARK_VARIANT];
  return shapes.map((s, i) => {
    switch (s.t) {
      case 'c':
        return <circle key={i} cx={s.cx} cy={s.cy} r={s.r} fill={color}/>;
      case 'e':
        return <path key={i} d={s.d} fill={color} fillRule="evenodd"/>;
      case 's':
        return (
          <path key={i} d={s.d} fill="none" stroke={color} strokeWidth={s.w}
            strokeLinecap="round" strokeLinejoin="round"/>
        );
      case 'r':
        return (
          <path key={i} d={s.d} fill={color} stroke={color} strokeWidth={s.w}
            strokeLinejoin="round"/>
        );
      default:
        return <path key={i} d={s.d} fill={color}/>;
    }
  });
}

// Standalone brand mark. Transparent ground; the surface shows through.
export function DysonMark({ size = 24, color = DYSON_BLUE, title = 'Dyson', variant, ...rest }) {
  const appVariant = useBrandMarkVariant();
  return (
    <svg
      width={size}
      height={size}
      viewBox="-112 -112 224 224"
      role="img"
      aria-label={title}
      style={{ display: 'block' }}
      {...rest}
    >
      <Glyph color={color} variant={variant ?? appVariant}/>
    </svg>
  );
}

// The Computer-kind mark: the Dyson mark framed inside a monitor. The monitor
// inherits currentColor (theme ink) so it reads on any card; the mark keeps
// its brand blue.
export function ComputerMark({ size = 24, color = DYSON_BLUE, title = 'Dyson Computer', variant, ...rest }) {
  const appVariant = useBrandMarkVariant();
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 240 224"
      role="img"
      aria-label={title}
      style={{ display: 'block' }}
      {...rest}
    >
      <rect x="16" y="8" width="208" height="152" rx="18" fill="none" stroke="currentColor" strokeWidth="11"/>
      <rect x="108" y="160" width="24" height="26" fill="currentColor"/>
      <rect x="74" y="186" width="92" height="14" rx="7" fill="currentColor"/>
      <g transform="translate(120,84) scale(0.62)">
        <Glyph color={color} variant={variant ?? appVariant}/>
      </g>
    </svg>
  );
}
