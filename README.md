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
