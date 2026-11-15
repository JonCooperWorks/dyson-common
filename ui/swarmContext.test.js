import { describe, expect, test, vi } from 'vitest';
import {
  getSwarmContext, loadSwarmContext, parseSwarmContext, safeName, swarmAgentHref, swarmHref,
} from './swarmContext.js';

const GOOD = {
  apex_url: 'https://swarm.example.com/',
  instance: { id: 'tars-1a2b', name: 'TARS' },
  agents: [
    { id: 'case-9', name: 'Case', open_url: 'https://case-9.swarm.example.com/' },
    { id: 'tars-1a2b', name: 'TARS', open_url: 'https://tars-1a2b.swarm.example.com/' },
  ],
};

function response(body, { ok = true, type = 'application/json' } = {}) {
  return { ok, headers: { get: () => type }, json: async () => body };
}

describe('parseSwarmContext', () => {
  test('keeps a well-formed answer', () => {
    const ctx = parseSwarmContext(GOOD);
    expect(ctx.apexUrl).toBe('https://swarm.example.com/');
    expect(ctx.instance).toEqual({ id: 'tars-1a2b', name: 'TARS' });
    expect(ctx.agents.map(a => a.openUrl)).toEqual([
      'https://case-9.swarm.example.com/', 'https://tars-1a2b.swarm.example.com/',
    ]);
    expect(swarmAgentHref(ctx)).toBe('https://swarm.example.com/#/i/tars-1a2b');
    expect(swarmHref(ctx, 'settings/billing')).toBe('https://swarm.example.com/#/settings/billing');
    expect(swarmHref(ctx, 'javascript:alert(1)')).toBeNull();
    expect(swarmHref(ctx, '../x')).toBeNull();
  });

  test('refuses anything that is not a bare https apex', () => {
    for (const apex of ['http://swarm.example.com/', 'javascript:alert(1)', 'https://swarm.example.com/x',
      'https://u:p@swarm.example.com/', 'https://swarm.example.com/?q', '//swarm.example.com/', 42, null]) {
      expect(parseSwarmContext({ ...GOOD, apex_url: apex })).toBeNull();
    }
    expect(parseSwarmContext({ ...GOOD, instance: { id: '../x', name: 'x' } })).toBeNull();
    expect(parseSwarmContext(null)).toBeNull();
  });

  test('drops agents whose address is not the one Swarm builds', () => {
    const ctx = parseSwarmContext({
      ...GOOD,
      agents: [
        { id: 'a-1', name: 'ok', open_url: 'https://a-1.swarm.example.com/' },
        { id: 'a-2', name: 'elsewhere', open_url: 'https://evil.example.net/' },
        { id: 'a-3', name: 'other id', open_url: 'https://a-1.swarm.example.com/' },
        { id: 'a-4', name: 'path', open_url: 'https://a-4.swarm.example.com/#/c/x' },
        { id: 'a-5', name: 'js', open_url: 'javascript:alert(1)' },
        { id: 'A_6', name: 'bad id', open_url: 'https://A_6.swarm.example.com/' },
        { id: 'a-7', name: 'http', open_url: 'http://a-7.swarm.example.com/' },
      ],
    });
    expect(ctx.agents.map(a => a.id)).toEqual(['a-1']);
  });

  test('names are plain, stripped and capped', () => {
    expect(safeName('‮evil\u0007 name')).toBe('evil name');
    expect(safeName('   ', 'fallback')).toBe('fallback');
    expect(Array.from(safeName('x'.repeat(500))).length).toBe(120);
    const ctx = parseSwarmContext({ ...GOOD, instance: { id: 'tars-1a2b', name: '⁦' } });
    expect(ctx.instance.name).toBe('tars-1a2b');
  });
});

describe('loadSwarmContext', () => {
  test('asks the same-origin route with cookies and no cache', async () => {
    const fetch = vi.fn(async () => response(GOOD));
    const ctx = await loadSwarmContext({ fetch });
    expect(ctx.instance.id).toBe('tars-1a2b');
    expect(fetch).toHaveBeenCalledWith('/_swarm/context', expect.objectContaining({
      credentials: 'same-origin', cache: 'no-store',
    }));
  });

  test('is null when Swarm does not answer (self-hosted Dyson)', async () => {
    expect(await loadSwarmContext({ fetch: async () => response(null, { ok: false }) })).toBeNull();
    expect(await loadSwarmContext({ fetch: async () => response('<html>', { type: 'text/html' }) })).toBeNull();
    expect(await loadSwarmContext({ fetch: async () => { throw new Error('offline'); } })).toBeNull();
  });

  test('getSwarmContext asks once per page', async () => {
    const fetch = vi.fn(async () => response(GOOD));
    const first = getSwarmContext({ fetch, reset: true });
    const second = getSwarmContext({ fetch });
    expect(await first).toEqual(await second);
    expect(fetch).toHaveBeenCalledTimes(1);
  });
});
