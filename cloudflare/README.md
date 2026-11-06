# dyson-cloudflare

Cloudflare calls shared by Dyson products, taken out of
`dyson-inference-honeypot`'s `dns.rs` and `cloudflare.rs`. It has no product
database, key list, scheduler, authorization, environment lookup or policy
about where a name should point.

- `Client`: one reqwest client (no redirects, 20s timeout, caller's user
  agent) for any number of tokens. Every call takes the `Token` it acts with.
  `with_endpoint` points it at a stand-in; products should honour such an
  override in debug builds only.
- Tokens: `Token` (trimmed, at most 256 printable ASCII characters, wiped on
  drop, `Debug` shows `hint()` only) and `verify` (the account-owned endpoint
  first when an account id is given, then the user-owned one).
- Zones: `zones` lists every zone a token reaches (50 per page, 20 pages at
  most). `zone_for` picks a host's zone by longest suffix.
- Records: `records` (every type at exactly one name), `ensure_free`
  (`Error::Taken` when the name has any record), `create_record`,
  `update_record` and `delete_record` for A, AAAA and CNAME (`NewRecord`).
- Rules: `ensure_rule` brings the rule with a given `ref` in a phase entry
  point to what is wanted, or removes it. Other rules in the phase are left as
  they are, and nothing is written when `same_rule` says the stored one already
  matches. `hosts_expression` builds `(http.host in {...})` with quotes and
  backslashes escaped.
- Zone settings: `managed_headers`, `enable_managed_header`,
  `bot_fight_mode`.
- `reconcile::addresses` / `reconcile::cname`: bring one name's records to
  what is wanted. They touch only records the caller owns: `Owned::Ids` (the
  ids it recorded, for zones shared with their owners) or `Owned::All` (every
  record of the reconciled type at the name, which is the honeypot's model). A
  new address is made before a stale one is deleted, so the name keeps
  answering. A CNAME is changed in place, never made beside the old one.
- `ai`: Workers AI account URLs. `Account::parse` accepts every dashboard
  form (`.../accounts/<id>`, plus `/ai`, `/ai/run` or `/ai/v1`, and
  `?gateway=`). It also builds the request, catalogue, token-verify and
  credit-balance URLs and the `cf-aig-metadata` value. These are pure
  functions that products send with their own clients.

Errors are typed (`Error::{Unreachable, Api, NotFound, Taken, Invalid,
Malformed}`). `Api` carries Cloudflare's error codes and messages; a 2xx
whose envelope says `success: false` is still `Api`. Zone, record, account
and phase ids are checked before they go into a path. Transport errors leave
the URL out.

No reqwest types are public, so products on another reqwest version
(the honeypot is on 0.12) can use the crate. The client is built with an
explicit Rustls provider and the bundled webpki roots: `ring` by default, or
`default-features = false, features = ["aws-lc-rs"]` for applications that
use AWS-LC, so an application keeps one provider. No process-wide provider
is installed.

Run `cargo test -p dyson-cloudflare` and
`cargo clippy -p dyson-cloudflare --all-targets -- -D warnings` from the
workspace. The tests use a local stand-in for Cloudflare's API and need
localhost sockets. `cargo test -p dyson-cloudflare -- --ignored` also makes
one real TLS call to api.cloudflare.com with a made-up token.
