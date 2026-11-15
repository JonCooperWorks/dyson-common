/* The Dyson chat app's way back into Swarm (UX spec 2.4, 5.3 rule 7).
 *
 * When a Dyson agent runs under Swarm, the Swarm proxy answers
 * `GET /_swarm/context` on the agent's own host itself (it is never
 * forwarded to the agent): `{ apex_url, instance: { id, name },
 * agents: [{ id, name, open_url }] }`. A self-hosted Dyson has no such
 * route, so the fetch fails and the app shows no Swarm bar.
 *
 * Everything is validated before it becomes a link: the apex must be a bare
 * https origin, IDs must be DNS labels, and an agent's chat address must be
 * exactly `https://<id>.<apex host>/`, the shape Swarm builds. Anything else
 * is dropped, so even a tampered answer can only point back into Swarm. */

export const SWARM_CONTEXT_PATH = '/_swarm/context';

const ID = /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/;
const MAX_NAME = 120;
// C0/C1 controls and the bidirectional overrides/isolates (spec 5.3 rule 9).
// eslint-disable-next-line no-control-regex
const UNSAFE = /[\u0000-\u001f\u007f-\u009f؜‎‏‪-‮⁦-⁩]/g;

export function isAgentId(value) {
  return typeof value === 'string' && ID.test(value);
}

/// Plain display text: control and bidi characters stripped, capped at 120.
export function safeName(value, fallback = '') {
  const text = String(value ?? '').replace(UNSAFE, '').trim();
  return (text ? Array.from(text).slice(0, MAX_NAME).join('') : fallback);
}

function apexOrigin(value) {
  if (typeof value !== 'string') return null;
  let url;
  try { url = new URL(value); } catch { return null; }
  if (url.protocol !== 'https:' || url.username || url.password) return null;
  if ((url.pathname !== '/' && url.pathname !== '') || url.search || url.hash) return null;
  return url;
}

function chatUrl(value, id, apex) {
  if (typeof value !== 'string') return null;
  let url;
  try { url = new URL(value); } catch { return null; }
  if (url.protocol !== 'https:' || url.username || url.password || url.search || url.hash) return null;
  if (url.pathname !== '/' || url.port !== apex.port) return null;
  if (url.hostname !== `${id}.${apex.hostname}`) return null;
  return `${url.origin}/`;
}

/// Validated context, or null when the answer isn't usable.
export function parseSwarmContext(raw) {
  if (!raw || typeof raw !== 'object') return null;
  const apex = apexOrigin(raw.apex_url);
  const instance = raw.instance;
  if (!apex || !instance || !isAgentId(instance.id)) return null;
  const agents = [];
  for (const a of Array.isArray(raw.agents) ? raw.agents : []) {
    if (!a || !isAgentId(a.id)) continue;
    const openUrl = chatUrl(a.open_url, a.id, apex);
    if (!openUrl) continue;
    agents.push({ id: a.id, name: safeName(a.name, a.id), openUrl });
  }
  return {
    apexUrl: `${apex.origin}/`,
    instance: { id: instance.id, name: safeName(instance.name, instance.id) },
    agents,
  };
}

/// `<apex>/#/i/<id>`: this agent's page in Swarm.
export function swarmAgentHref(context, id = context?.instance?.id) {
  if (!context || !isAgentId(id)) return null;
  return `${context.apexUrl}#/i/${encodeURIComponent(id)}`;
}

/// `<apex>/#/<route>` for a fixed Swarm route (`agents`, `settings/billing`,
/// `i/<id>/activity`). Only lower-case path segments are accepted.
export function swarmHref(context, route) {
  if (!context || typeof route !== 'string' || !/^[a-z0-9-]+(?:\/[a-z0-9-]+)*$/.test(route)) return null;
  return `${context.apexUrl}#/${route}`;
}

/// Ask the Swarm proxy for the context. Resolves to null on any failure
/// (self-hosted Dyson, signed out, network error, bad shape); never throws.
export async function loadSwarmContext({ fetch: fetchImpl = globalThis.fetch, signal } = {}) {
  if (typeof fetchImpl !== 'function') return null;
  try {
    const r = await fetchImpl(SWARM_CONTEXT_PATH, {
      credentials: 'same-origin',
      cache: 'no-store',
      headers: { Accept: 'application/json' },
      signal,
    });
    if (!r || !r.ok) return null;
    const type = r.headers?.get?.('content-type') || '';
    if (!type.includes('application/json')) return null;
    return parseSwarmContext(await r.json());
  } catch {
    return null;
  }
}

let shared = null;

/// One request per page load, shared by every caller (the header bar, the
/// Audit link, the balance error). `reset` is for tests.
export function getSwarmContext({ fetch: fetchImpl, reset = false } = {}) {
  if (reset) shared = null;
  if (!shared) shared = loadSwarmContext(fetchImpl ? { fetch: fetchImpl } : {});
  return shared;
}
