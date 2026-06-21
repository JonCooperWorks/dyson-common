# dyson-common

Shared wire contracts and primitives for [`dyson`](https://github.com/JonCooperWorks/dyson)
and [`dyson-swarm`](https://github.com/JonCooperWorks/dyson-swarm). Both repos
are independent checkouts that previously kept hand-synced copies of these types
— copies that drifted. Defining them once here makes drift a compile error.

Leaf crate by design: pure types + serde + small pure helpers. The reqwest-based
OAuth flow is behind the `oauth-client` feature; everything else is dependency-light.

## Modules

| Module | What |
|---|---|
| `tokens` | `typed_token!` macro, `BadToken`, validation, the `pt_`/`it_`/`st_`/`ses_` bearer types |
| `oauth` | RFC 6749/7591/8414/9728 wire DTOs; flow client under `oauth-client` |
| `cost` | `RecentCostCall` — the `/v1/internal/audit/calls` cost-row contract |
| `feedback` | `FeedbackRating` (+ score, + emoji mapping), `FeedbackEntry` |
| `marketplace` | published skill-catalog DTOs (`CatalogSkill`, `SkillPackageBody`, …) |

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

- `ui/Modal.jsx`, `ui/Combobox.jsx`, `ui/useEscapeKey.js` — the shared
  `<Modal>` / `<Combobox>` / `useEscapeKey` primitives (+ colocated tests).
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
"dyson-common-ui": "git+ssh://git@github.com/JonCooperWorks/dyson-common.git#ui-v0.1.0"
```

```js
import { Modal, Combobox, useEscapeKey } from 'dyson-common-ui';
import 'dyson-common-ui/tokens.css';      // canonical tokens (the shared look)
import 'dyson-common-ui/components.css';   // <Modal>/<Combobox> styles
```

Each consumer's Vite config sets `resolve.dedupe: ['react','react-dom']`
so the peer React resolves to a single instance.  Bumping the UI: rebuild
`dist/`, tag `ui-vX.Y.Z` here, bump the tag in both web apps, run all suites.
