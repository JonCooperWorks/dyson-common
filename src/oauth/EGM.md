# dyson_common::oauth

## Overview

OAuth 2.0 / MCP-auth wire DTOs (RFC 6749, 7591, 8414, and 9728) shared by
`dyson` (agent-side MCP OAuth) and `dyson-swarm` (server-side MCP auth). The
DTOs are always compiled. The stateless reqwest transport in `client.rs` is
behind the `oauth-client` feature.

## Interface

### DTOs (`mod.rs`, always compiled)

- `AuthMetadata` [mod.rs:AuthMetadata]
- `ProtectedResourceMetadata` [mod.rs:ProtectedResourceMetadata]
- `DcrRequest`, including an optional `scope` [mod.rs:DcrRequest]
- `DcrResponse` [mod.rs:DcrResponse]
- `TokenResponse`, with optional `token_type` and `scope` [mod.rs:TokenResponse]
- `PkceChallenge` [mod.rs:PkceChallenge]

### Transport (`client.rs`, feature `oauth-client`)

- `OAuthError` [client.rs:OAuthError]
- `OAuthError::redacted(redact, with_body)`: lets the caller control error formatting [client.rs:OAuthError::redacted]
- `generate_pkce()` and `generate_state()` [client.rs:generate_pkce] [client.rs:generate_state]
- `as_metadata_url`: the RFC 8414 well-known URL [client.rs:as_metadata_url]
- `protected_resource_metadata_urls`: the RFC 9728 URLs to try, path-scoped form first, then origin root [client.rs:protected_resource_metadata_urls]
- `canonical_resource`: the RFC 8707 resource, which is the URL's origin [client.rs:canonical_resource]
- `build_auth_url`: the `/authorize` URL with PKCE, state, and an optional `resource` [client.rs:build_auth_url]
- Async calls:
  - `fetch_as_metadata` [client.rs:fetch_as_metadata]
  - `fetch_protected_resource` [client.rs:fetch_protected_resource]
  - `register_client` (DCR) [client.rs:register_client]
  - `exchange_code` [client.rs:exchange_code]
  - `refresh_token` [client.rs:refresh_token]

## Invariants

- The transport does no SSRF gating and no redaction. Callers validate endpoints around these calls and format errors through `OAuthError::redacted` (module docs) [client.rs]
- Response bodies are read in chunks and rejected once they exceed `MAX_OAUTH_RESPONSE_BYTES` (256 KiB) [client.rs:MAX_OAUTH_RESPONSE_BYTES]
- A non-success HTTP status is returned as an `OAuthError`, never parsed as a DTO [client.rs:fetch_protected_resource] [client.rs:register_client]
- `build_auth_url` leaves out `scope` when the scope slice is empty [client.rs:build_auth_url]
- The transport is stateless: no token storage, refresh scheduling, or callback server [client.rs]

## Side Effects

- HTTP GET to metadata endpoints and HTTP POST to the registration and token endpoints, using the caller-supplied `reqwest::Client` [client.rs:fetch_as_metadata] [client.rs:register_client] [client.rs:exchange_code] [client.rs:refresh_token]
- Uses OS randomness (`rand`) for the PKCE verifier and state [client.rs:generate_pkce] [client.rs:generate_state]

## Verification

- Run: `cargo test --features oauth-client` (from `dyson-common/`)
