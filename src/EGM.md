# dyson-common (src)

## Overview

Library source for the root `dyson-common` crate. It is the single definition
of the wire contracts and small pure helpers that cross the `dyson` and
`dyson-swarm` boundary, plus the Glitch and security-tester contracts. By
design the crate is a leaf: types, serde, and pure helpers. There is no
database, no HTTP server, and no per-repository trait types. Optional
dependencies are feature-gated:

- `oauth-client`: the OAuth flow transport
- `oidc`: JWKS key construction
- `glitch-scope`: the Glitch scope compiler

## Interface

### Modules (`lib.rs`)

- Always compiled: `auth`, `contracts`, `cost`, `date`, `feedback`, `jsonrpc`, `marketplace`, `net`, `oauth`, `security_tester`, `state_sync`, `telegram_media`, `tokens`, `ui`, `util` [lib.rs]
- `glitch_scope`: only with `#[cfg(feature = "glitch-scope")]` [lib.rs]
- `oidc`: only with `#[cfg(feature = "oidc")]` [lib.rs]
- `oauth`: OAuth / MCP-auth DTOs and the feature-gated client. See [./oauth/EGM.md](./oauth/EGM.md)

### Auth (`auth.rs`)

- `strip_bearer(header_value) -> Option<&str>` [auth.rs:strip_bearer]

### Tokens (`tokens.rs`)

- Token types: `ProxyToken` (`pt_`), `IngestToken` (`it_`), `StateSyncToken` (`st_`), `SessionToken` (`ses_`), `ApiKeyToken` (`sk_`), `McpExposureToken` (`mcp_`) [tokens.rs:typed_token]
- `typed_token!` generates each type with `PREFIX`, `parse`, `as_str`, and `into_inner` [tokens.rs:typed_token]
- `validate(token, prefix)` [tokens.rs:validate]
- `BadToken` [tokens.rs:BadToken]
- `TOKEN_BODY_HEX_LEN` (32) and the `*_TOKEN_PREFIX` constants [tokens.rs:TOKEN_BODY_HEX_LEN]

### Contract constants (`contracts.rs`)

- Header constants:
  - `DYSON_CONFIGURE_HEADER` (`X-Swarm-Configure`)
  - `DYSON_CSRF_HEADER` (`X-Dyson-CSRF`)
  - `SWARM_LLM_AUDIT_ID_HEADER`

  [contracts.rs:DYSON_CONFIGURE_HEADER]
- Admin endpoint constants `DYSON_ADMIN_*_ENDPOINT` [contracts.rs:DYSON_ADMIN_CONFIGURE_ENDPOINT]
- Swarm paths:
  - `SWARM_INTERNAL_ARTEFACT_INGEST_PATH` (`/v1/internal/ingest/artefact`)
  - `SWARM_INTERNAL_STATE_FILE_PATH` (`/v1/internal/state/file`)
  - `SWARM_SHARE_MINT_PATH` (`/_swarm/share-mint`)

  [contracts.rs:SWARM_INTERNAL_STATE_FILE_PATH]
- `guest_security` module (Falco telemetry over vsock) [contracts.rs:guest_security]:
  - `VSOCK_PORT` (9473) and `HOST_CID` (2)
  - rule and field names
  - `MAX_ARGUMENTS` (64) and `MAX_ARGUMENT_BYTES` (4096)
  - `SWARM_INGEST_PATH` (`/v1/internal/ingest/security-events`)
  - `PRIORITY_SEVERITY` and `severity(priority)`

### Shared DTOs

- `RecentCostCall`: the `/v1/internal/audit/calls` row [cost.rs:RecentCostCall]
- `FeedbackRating`, seven variants from `Terrible` to `Excellent`, with `score()` and `from_emoji()` [feedback.rs:FeedbackRating]
- `FeedbackEntry` [feedback.rs:FeedbackEntry]
- JSON-RPC 2.0 envelope: `JsonRpcRequest::new`, `JsonRpcNotification::new`, `JsonRpcResponse`, `JsonRpcError` [jsonrpc.rs:JsonRpcRequest] [jsonrpc.rs:JsonRpcResponse]
- Skill catalogue: `SkillPackageContent`, `CatalogSkill`, `CatalogSkillAuthor`, `CatalogError`, `SkillPackageDetail`, `SkillPackageBody` [marketplace.rs:CatalogSkill]

### Dates (`date.rs`)

- `unix_to_ymd(secs)` [date.rs:unix_to_ymd]
- `days_from_civil(year, month, day)` [date.rs:days_from_civil]

### Network policy (`net.rs`)

- `is_private_v4`, `is_private_v6`, `is_public_target`: SSRF address predicates [net.rs:is_private_v4] [net.rs:is_private_v6] [net.rs:is_public_target]
- `is_metadata_host` [net.rs:is_metadata_host]
- `host_from_url` [net.rs:host_from_url]

### OIDC (`oidc.rs`, feature `oidc`)

- `Jwk` [oidc.rs:Jwk]
- `JwkSet::find(kid)` [oidc.rs:JwkSet]
- `decoding_key_from_jwk(&Jwk) -> Option<DecodingKey>` [oidc.rs:decoding_key_from_jwk]

### Glitch scope (`glitch_scope.rs`, feature `glitch-scope`)

- `SCOPE_COMPILER_VERSION` (1) [glitch_scope.rs:SCOPE_COMPILER_VERSION]
- `ScopeDeclaration` [glitch_scope.rs:ScopeDeclaration]
- `CompiledScope`, with `permits_target(url)` and `permits(url, resolved)` [glitch_scope.rs:CompiledScope]
- `ScopeError` [glitch_scope.rs:ScopeError]
- `declared_hosts` [glitch_scope.rs:declared_hosts]
- `compile_scope` [glitch_scope.rs:compile_scope]
- `normalize_host` [glitch_scope.rs:normalize_host]

### Security tester (`security_tester.rs`)

- Constants:
  - `API_VERSION` (1)
  - `INSTANCE_KIND` (`security-tester.v1`)
  - `CAPABILITY_FLAG`
  - `HARNESS_PROFILE` (`byo:glitch`)
  - `MODEL_ROUTE` (`swarm-cyber`)
  - `CONTROL_MEDIA_TYPE`

  [security_tester.rs:API_VERSION]
- Control-plane DTOs: `HarnessProfile`, `ModelRoute`, `ConfigureRequest`, `ReadinessState`, `HealthResponse`, `ControlAction`, `ActionRequest`, `ActionResponse`, `SnapshotRequest`, `SnapshotResponse`, `SnapshotEnvelope`, `RestoreRequest`, `InstanceStatus`, `ReleaseIdentity` [security_tester.rs:ConfigureRequest] [security_tester.rs:InstanceStatus]

### State sync (`state_sync.rs`)

- `STATE_NAMESPACE_WORKSPACE` and `STATE_NAMESPACE_CHATS` [state_sync.rs:STATE_NAMESPACE_WORKSPACE]
- Sandbox env var names: `ENV_PROXY_URL`, `ENV_PROXY_TOKEN`, `ENV_INSTANCE_ID`, `ENV_STATE_SYNC_URL`, `ENV_STATE_SYNC_TOKEN` [state_sync.rs:ENV_PROXY_URL]
- `StateFileEnvelope` [state_sync.rs:StateFileEnvelope]
- `is_durable_state_path` [state_sync.rs:is_durable_state_path]
- `has_hidden_or_unclean_component` [state_sync.rs:has_hidden_or_unclean_component]
- `is_zero_byte_chat_transcript` [state_sync.rs:is_zero_byte_chat_transcript]

### Telegram media (`telegram_media.rs`)

- Compatibility re-export of `dyson_telegram::media::*`. New consumers should depend on `dyson-telegram` (see [../telegram/EGM.md](../telegram/EGM.md)) [telegram_media.rs]

### UI theme (`ui.rs`)

- `THEME_COOKIE` (`dyson-theme`) [ui.rs:THEME_COOKIE]
- `DESIGN_TOKENS_CSS`, which is `include_str!("../ui/tokens.css")` [ui.rs:DESIGN_TOKENS_CSS]
- `ThemeMode::{System, Light, Dark}`, with `parse`, `html_attr`, and `theme_color_meta` [ui.rs:ThemeMode]
- `first_paint_theme_script(storage_key)` [ui.rs:first_paint_theme_script]

### Utilities (`util.rs`)

- `ct_eq_bytes` and `ct_eq_str`: constant-time comparison [util.rs:ct_eq_bytes] [util.rs:ct_eq_str]
- `env_truthy` [util.rs:env_truthy]
- `backoff_ms` and `MAX_BACKOFF_SHIFT` (6) [util.rs:backoff_ms]

## Invariants

### Tokens

- A typed token is only built through `parse`. Its `Deserialize` impl also goes through `parse`, so an unvalidated string can never deserialize into a typed token [tokens.rs:typed_token]
- `validate` requires the exact prefix followed by exactly 32 ASCII hex characters. It reports `TooShort`, `TooLong`, `WrongPrefix`, or `BadBody` [tokens.rs:validate]

### Auth

- `strip_bearer` accepts only the `Bearer ` or `bearer ` scheme, trims whitespace, and returns `None` for an empty token [auth.rs:strip_bearer]

### State sync

- `is_durable_state_path` rejects paths with dotfile, empty, non-UTF-8, `..`, or root components [state_sync.rs:is_durable_state_path] [state_sync.rs:has_hidden_or_unclean_component]
- In the `chats` namespace every clean path is durable [state_sync.rs:is_durable_state_path]
- In the `workspace` namespace these are durable [state_sync.rs:should_sync_workspace_path] [state_sync.rs:has_extension]:
  - top-level `.md` files
  - `memory/**/*.md`
  - `kb/**` and `skills/**`
  - `channels/<c>/` with either `*.md` or `_audit.jsonl`, or `memory/**/*.md`
  - the `.md` extension is matched case-insensitively

### Glitch scope

- `compile_scope` is pure and never resolves DNS. Non-IP hosts must have non-empty `resolved_addresses` supplied by the caller [glitch_scope.rs:compile_scope]
- `compile_scope` rejects [glitch_scope.rs:compile_scope]:
  - schemes other than `http`, `https`, `ws`, `wss`
  - URLs with embedded credentials
  - excluded hosts
  - ports other than 80 and 443
  - an empty host-and-CIDR set
- The digest is SHA-256 over the canonical serde serialisation of (hosts, addresses, cidrs, ports, schemes). `SCOPE_COMPILER_VERSION` and the golden digest change only together, as a coordinated release with Glitch and Swarm (module docs) [glitch_scope.rs:compile_scope] [glitch_scope.rs:SCOPE_COMPILER_VERSION]
- `CompiledScope::permits` requires `permits_target` (scheme, host, and port in scope, no credentials) and that the resolved IP is in `addresses` or one of `cidrs` [glitch_scope.rs:CompiledScope::permits]

### Security tester

- The DTOs carry configuration references and summarised status only. Target and provider credentials, raw evidence, and CA private keys are never valid members (module docs) [security_tester.rs]

### Network and utilities

- `is_private_v4` covers [net.rs:is_private_v4]:
  - loopback, RFC 1918, link-local, broadcast, unspecified, and multicast
  - CGNAT `100.64/10`
  - `0/8`
  - `>= 240/4`
- `is_public_target` additionally excludes documentation, benchmarking, 6to4, NAT64, Teredo, ORCHID, and deprecated site-local ranges [net.rs:is_public_target]
- `backoff_ms` is `base_ms * 2^min(attempt, 6)`, saturating, with no jitter [util.rs:backoff_ms]
- `env_truthy` accepts only `1`, `true`, `TRUE`, `yes`, `YES` [util.rs:env_truthy]

## Side Effects

- `env_truthy` reads process environment variables [util.rs:env_truthy]
- `compile_scope` generates a random UUIDv4 `version_id` [glitch_scope.rs:compile_scope]
- `ui/tokens.css` is embedded at compile time [ui.rs:DESIGN_TOKENS_CSS]
- HTTP calls exist only in `oauth::client` (feature `oauth-client`). See [./oauth/EGM.md](./oauth/EGM.md)
- Every other module is pure.

## Verification

- Run: `cargo test --features oauth-client,oidc,glitch-scope` (from `dyson-common/`)
