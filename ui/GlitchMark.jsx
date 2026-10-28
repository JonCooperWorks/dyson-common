import React from 'react';

// Glitch's mark: the Dyson orbits made slightly wrong (docs/plans/assets/
// glitch-mark in dyson.myprivate.network). `bleed` carries the scanline tear
// with red showing through; at 24 px and below that turns to noise, so small
// marks use `drift`, the same slipped orbits without the tear.
const NIGHT = '#0f1115';
const DRIFT_MAX_SIZE = 24;

const ORBITS = (
  <>
    <path d="M53.24,-30.74 A61.48,61.48 0 1 1 -48.45,-37.85" fill="none" strokeWidth="14.84"/>
    <path d="M-52.8,84.5 A99.64,99.64 0 1 1 74.05,-66.67" fill="none" strokeWidth="12.72" strokeLinecap="butt"/>
    <circle cx="-37.85" cy="-48.45" r="13.78" stroke="none"/>
    <circle cx="86" cy="-76" r="12" stroke="none"/>
    <circle cx="4.5" cy="3.5" r="27.56" stroke="none"/>
  </>
);

export function GlitchMark({ size = 28 }) {
  const id = React.useId().replace(/:/g, '');
  const drift = size <= DRIFT_MAX_SIZE;
  return (
    <svg className="glitch-mark" width={size} height={size} viewBox="0 0 512 512" aria-hidden="true">
      <rect width="512" height="512" rx="112" fill={NIGHT}/>
      {drift ? (
        <g transform="translate(256,256) scale(1.9)" fill="#fff" stroke="#fff" strokeLinecap="round">{ORBITS}</g>
      ) : (
        <>
          <defs>
            <clipPath id={`${id}a`}><rect x="-130" y="-130.4" width="260" height="126.8"/></clipPath>
            <clipPath id={`${id}b`}><rect x="-130" y="-4.4" width="260" height="9.8"/></clipPath>
            <clipPath id={`${id}c`}><rect x="-130" y="4.6" width="260" height="125.8"/></clipPath>
          </defs>
          <g transform="translate(256,256) scale(1.9)" fill="#fff" stroke="#fff" strokeLinecap="round">
            <g clipPath={`url(#${id}a)`}>{ORBITS}</g>
            <g clipPath={`url(#${id}b)`}><g transform="translate(-4.2 0)" fill="#ff3b4e" stroke="#ff3b4e" opacity=".9">{ORBITS}</g></g>
            <g clipPath={`url(#${id}b)`}><g transform="translate(7 0)">{ORBITS}</g></g>
            <g clipPath={`url(#${id}c)`}>{ORBITS}</g>
          </g>
        </>
      )}
    </svg>
  );
}
