# dyson-common

Shared wire contracts and primitives for [`dyson`](https://github.com/JonCooperWorks/dyson)
and [`dyson-swarm`](https://github.com/JonCooperWorks/dyson-swarm). Both repos
are independent checkouts that previously kept hand-synced copies of these types
— copies that drifted. Defining them once here makes drift a compile error.

Leaf crate by design: pure types + serde + small pure helpers. The reqwest-based
OAuth flow is behind the `oauth-client` feature; everything else is dependency-light.

The repository also contains the separate `dyson-inference-protocol` crate.
It owns the framework-neutral public inference contract: OpenAI Chat
Completions, Anthropic Messages, OpenAI Responses, model route classification,
SDK-shaped errors, and response/SSE translation. Hosts keep authentication,
billing, policy, provider credentials, and audit persistence in their own
application.

Its generic `ModelRouter<T>` is the same deterministic routing seam used by
Swarm: exact routes win over the longest trailing-star prefix, then regex
routes, then `*`. The payload is application-defined, so a honeypot can attach
decoy responses or capture policy while Swarm attaches failover hops and
billing policy.

## Modules

| Module | What |
|---|---|
| `tokens` | `typed_token!` macro, `BadToken`, validation, the `pt_`/`it_`/`st_`/`ses_` bearer types |
| `oauth` | RFC 6749/7591/8414/9728 wire DTOs; flow client under `oauth-client` |
| `cost` | `RecentCostCall` — the `/v1/internal/audit/calls` cost-row contract |
| `feedback` | `FeedbackRating` (+ score, + emoji mapping), `FeedbackEntry` |
| `marketplace` | published skill-catalog DTOs (`CatalogSkill`, `SkillPackageBody`, …) |

### Inference protocol

```toml
dyson-inference-protocol = { git = "https://github.com/JonCooperWorks/dyson-common", tag = "v0.1.12" }
```

Use `classify_route` at the HTTP edge, then pass the normalized OpenAI-shaped
request through the host's own security and accounting pipeline. The crate has
no knowledge of API keys, tenants, upstream credentials, or storage, so it can
also back an isolated honeypot without importing Swarm.

## Use

```toml
# DTOs only
dyson-common = { git = "https://github.com/JonCooperWorks/dyson-common", tag = "v0.1.0" }
# + the OAuth flow client (reqwest)
dyson-common = { git = "https://github.com/JonCooperWorks/dyson-common", tag = "v0.1.0", features = ["oauth-client"] }
```

Pin to a tag or `rev`, never a bare branch. Bumping a contract: tag here, bump
the tag in both repos, run both test suites.

## SSRF

The OAuth flow client takes no internal-host policy of its own — every networked
call accepts an `allow_url: impl Fn(&str) -> bool` predicate. Each consumer plugs
in its own (dyson's SSRF predicates, swarm's egress CIDR).

## Frontend (`dyson-common-ui`)

The same single-source idea, for the web UIs.  `dyson` and `dyson-swarm`
both render a React frontend; their shared primitives and design tokens
live here so the two apps look and behave the same instead of drifting.

- `ui/Modal.jsx`, `ui/Combobox.jsx`, `ui/useEscapeKey.js` — shared interaction
  primitives (+ colocated tests).
- `ui/GatewayPrimitives.jsx` — page headers, metrics, status badges, filter
  toolbars, responsive data tables, detail lists, and JSON inspectors for
  inference and observability products.
- `ui/Sorting.jsx`, `ui/sort.js` — sortable table heads that order the whole
  list (`useSorter` over one `sort=` string, `-col` for descending;
  `<SortTh>` with a menu for columns with several keys, `<SortPill>`,
  a phone `<SortPicker>`), and `sortRows` for lists sorted in the browser.
- `ui/Pager.jsx`, `ui/SegmentedControl.jsx` — one paging control for
  server- and client-paged lists (page jump, unknown end, rows-per-page kept
  per browser via `usePageSize`) and the segmented choice it uses.
- `ui/clipboard.js` — `copyText` answers copied / selected (for a manual ⌘C
  where the browser refuses) / failed; `copyFetched` copies text still being
  fetched without losing the click's user activation (Safari).
- `ui/buildWatch.js` + `dyson-common-ui/vite` — stale-tab detection: the
  build stamps an id into the bundle and `version.json`; `createBuildWatcher`
  polls it so an app can reload a tab left open across a deploy.
- `ui/useStored.js` — `useStored` (a localStorage-backed state that survives
  blocked storage) and `useDebounced`.
- `ui/tokens.css` — the canonical design tokens (colors, fonts, radius).
- `ui/components.css` — styles for the shared primitives.

Ships **pre-built ESM** (`dist/`, committed) with `react` external, so
consumers import plain JS over a git dependency — no JSX-in-`node_modules`
transpile step.  Rebuild after editing `ui/`:

```sh
npm install && npm run build && npm test
```

### Use

```jsonc
// package.json of each web app — pin to a tag, like the Rust dep
"dyson-common-ui": "git+ssh://git@github.com/JonCooperWorks/dyson-common.git#ui-v0.1.19"
```

```js
import { Modal, Combobox, useEscapeKey } from 'dyson-common-ui';
import 'dyson-common-ui/tokens.css';      // canonical tokens (the shared look)
import 'dyson-common-ui/components.css';   // <Modal>/<Combobox> styles
```

Shared component classes are prefixed `dc-` (`dc-pager`, `dc-seg`,
`dc-sort-*`) so they never collide with an app's own.

```js
// vite.config.js — stamp a build id for createBuildWatcher
import { buildIdPlugin } from 'dyson-common-ui/vite';
const buildId = buildIdPlugin();
export default defineConfig({ plugins: [react(), buildId.plugin], define: buildId.define });
```

Each consumer's Vite config sets `resolve.dedupe: ['react','react-dom']`
so the peer React resolves to a single instance.  Bumping the UI: rebuild
`dist/`, tag `ui-vX.Y.Z` here, bump the tag in both web apps, run all suites.
