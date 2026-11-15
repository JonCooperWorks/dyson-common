import React, { useEffect, useRef, useState } from 'react';
import { getSwarmContext, swarmAgentHref, swarmHref } from './swarmContext.js';

/* The Swarm bar (UX spec 2.4): the only Swarm chrome inside the Dyson chat
 * app. "‹ Swarm" goes to this agent's page in Swarm; the agent name opens a
 * list of the owner's other chat agents (each link opens that agent's chat)
 * and ends with "All agents". Every address comes from a validated
 * `parseSwarmContext` answer, which Swarm itself serves; names are text. */

function Chevron({ dir = 'left' }) {
  const d = dir === 'left' ? 'M10 3.5 5.5 8l4.5 4.5' : 'M4 6l4 4 4-4';
  return (
    <svg className="swarm-bar-icon" width="16" height="16" viewBox="0 0 16 16" fill="none"
      stroke="currentColor" strokeWidth="1.35" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={d}/>
    </svg>
  );
}

export function SwarmBar({ context, className = '' }) {
  const [open, setOpen] = useState(false);
  const buttonRef = useRef(null);
  const menuRef = useRef(null);
  const back = swarmAgentHref(context);
  const all = swarmHref(context, 'agents');

  useEffect(() => {
    if (!open) return undefined;
    const first = menuRef.current?.querySelector('a');
    first?.focus();
    const onDown = (e) => {
      if (menuRef.current?.contains(e.target) || buttonRef.current?.contains(e.target)) return;
      setOpen(false);
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [open]);

  if (!context || !back) return null;
  const current = context.instance;

  const close = (refocus) => {
    setOpen(false);
    if (refocus) buttonRef.current?.focus();
  };

  const onMenuKey = (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close(true);
      return;
    }
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    const links = Array.from(menuRef.current?.querySelectorAll('a') || []);
    if (!links.length) return;
    e.preventDefault();
    const at = links.indexOf(document.activeElement);
    const next = e.key === 'ArrowDown' ? (at + 1) % links.length : (at - 1 + links.length) % links.length;
    links[next].focus();
  };

  return (
    <nav className={`swarm-bar${className ? ` ${className}` : ''}`} aria-label="Swarm">
      <a className="swarm-bar-back" href={back} title="Back to this agent in Swarm">
        <Chevron/>
        <span>Swarm</span>
      </a>
      <span className="swarm-bar-switch">
        <button
          ref={buttonRef}
          type="button"
          className="swarm-bar-agent"
          aria-haspopup="true"
          aria-expanded={open}
          title={current.name}
          onClick={() => setOpen(v => !v)}
        >
          <span className="swarm-bar-name">{current.name}</span>
          <Chevron dir="down"/>
        </button>
        {open && (
          <div ref={menuRef} className="swarm-bar-menu" onKeyDown={onMenuKey}>
            <ul aria-label="Your chat agents">
              {context.agents.map(a => (
                <li key={a.id}>
                  <a
                    href={a.openUrl}
                    aria-current={a.id === current.id ? 'page' : undefined}
                    title={a.name}
                    onClick={() => setOpen(false)}
                  >
                    <span className="swarm-bar-name">{a.name}</span>
                  </a>
                </li>
              ))}
              {all && (
                <li className="swarm-bar-all">
                  <a href={all} onClick={() => setOpen(false)}>All agents</a>
                </li>
              )}
            </ul>
          </div>
        )}
      </span>
    </nav>
  );
}

/// The validated Swarm context, or null until (and unless) Swarm answers.
export function useSwarmContext() {
  const [context, setContext] = useState(null);
  useEffect(() => {
    let live = true;
    getSwarmContext().then(c => { if (live) setContext(c); });
    return () => { live = false; };
  }, []);
  return context;
}
