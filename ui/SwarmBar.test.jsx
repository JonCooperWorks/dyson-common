import React from 'react';
import { render, screen, fireEvent, cleanup } from '@testing-library/react';
import { afterEach, describe, expect, test } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { SwarmBar } from './SwarmBar.jsx';
import { parseSwarmContext } from './swarmContext.js';

const ctx = parseSwarmContext({
  apex_url: 'https://swarm.example.com/',
  instance: { id: 'tars-1', name: 'TARS' },
  agents: [
    { id: 'case-2', name: 'Case', open_url: 'https://case-2.swarm.example.com/' },
    { id: 'tars-1', name: 'TARS', open_url: 'https://tars-1.swarm.example.com/' },
  ],
});

afterEach(cleanup);


describe('SwarmBar', () => {
  test('renders nothing without a context', () => {
    const { container } = render(<SwarmBar context={null}/>);
    expect(container).toBeEmptyDOMElement();
  });

  test('links back to this agent in Swarm', () => {
    render(<SwarmBar context={ctx}/>);
    expect(screen.getByRole('link', { name: /Swarm/ })).toHaveAttribute('href', 'https://swarm.example.com/#/i/tars-1');
  });

  test('the agent name opens a switcher of chat agents, ending with All agents', () => {
    render(<SwarmBar context={ctx}/>);
    const button = screen.getByRole('button', { name: /TARS/ });
    expect(button).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(button);
    expect(button).toHaveAttribute('aria-expanded', 'true');
    const links = screen.getAllByRole('link').slice(1);
    expect(links.map(a => a.getAttribute('href'))).toEqual([
      'https://case-2.swarm.example.com/',
      'https://tars-1.swarm.example.com/',
      'https://swarm.example.com/#/agents',
    ]);
    expect(links[1]).toHaveAttribute('aria-current', 'page');
    expect(links[0]).toHaveFocus();
    fireEvent.keyDown(links[0], { key: 'ArrowDown' });
    expect(links[1]).toHaveFocus();
    fireEvent.keyDown(links[1], { key: 'Escape' });
    expect(button).toHaveAttribute('aria-expanded', 'false');
    expect(button).toHaveFocus();
  });

  test('agent names render as text, never markup', () => {
    const evil = parseSwarmContext({
      apex_url: 'https://swarm.example.com/',
      instance: { id: 'x-1', name: '<img src=x onerror=alert(1)>' },
      agents: [],
    });
    const { container } = render(<SwarmBar context={evil}/>);
    expect(container.querySelector('img')).toBeNull();
    expect(screen.getByText('<img src=x onerror=alert(1)>')).toBeInTheDocument();
  });
});
