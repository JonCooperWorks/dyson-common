//! `dyson-common` — the single source of truth for the wire contracts and
//! primitives that cross the `dyson` ↔ `dyson-swarm` boundary.
//!
//! Both repos are independent git checkouts; before this crate existed they
//! kept hand-synced copies of these types that had quietly drifted (a dropped
//! OAuth `token_type`, an added DCR `scope`, a token `Deserialize` that skipped
//! validation, parallel feedback/cost/catalog DTOs). Defining them once here
//! makes drift a compile error instead of a silent protocol break.
//!
//! Scope is deliberately a leaf: pure types + serde + small pure helpers. The
//! reqwest-bearing OAuth flow lives behind the `oauth-client` feature so a
//! consumer that only needs the DTOs pays for nothing else. No DB, no HTTP
//! server, no per-repo error or trait types — those stay in each repo.
//!
//! ## Modules
//! - [`tokens`] — the `typed_token!` macro, [`tokens::BadToken`], validation,
//!   and the four prefixed bearer types (`pt_`/`it_`/`st_`/`ses_`).
//! - [`oauth`] — RFC 6749/7591/8414/9728 wire DTOs (+ the flow client under
//!   `oauth-client`).
//! - [`cost`] — the `/v1/internal/audit/calls` cost-row wire DTO.
//! - [`feedback`] — the conversation feedback rating + entry.
//! - [`marketplace`] — the published skill-catalog wire DTOs.
//! - [`state_sync`] — the durable-state predicate, upload envelope, and
//!   sandbox env-var names for the state mirror.

pub mod auth;
pub mod cost;
pub mod feedback;
pub mod marketplace;
pub mod net;
pub mod oauth;
#[cfg(feature = "oidc")]
pub mod oidc;
pub mod state_sync;
pub mod tokens;
