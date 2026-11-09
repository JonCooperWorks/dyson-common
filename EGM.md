# dyson-common Blueprint

## System Overview

`dyson-common` is a git submodule. It holds the shared wire contracts and
primitives for `dyson` and `dyson-swarm`, so that drift between the two
independent repositories becomes a compile error instead of a silent protocol
break. It has two halves.

### Rust workspace

The root crate is `dyson-common`, at version `0.1.13` in `Cargo.toml`. The
workspace members are `telegram`, `inference-protocol`, `secrets`, and
`traces`.

The root crate is a leaf by design: DTOs, serde, and pure helpers. Its
optional dependencies sit behind three features:

- `oauth-client`
- `oidc`
- `glitch-scope`

The workspace `[patch.crates-io]` pins `vectorscan-rs` and `vectorscan-rs-sys`
to the Kingfisher `v2.7.0` tag. Every workspace that enables the `kingfisher`
feature of `dyson-secrets` must repeat this patch.

### npm package

The npm package is `dyson-common-ui` (`package.json`). It contains React 19 UI
primitives from `ui/`, bundled by esbuild into `dist/index.js`, plus
`ui/tokens.css`, `ui/components.css`, and `ui/vite.js`.

## Local Interface & Invariants

- `vitest.config.js`: the vitest config, which uses the React plugin and the `jsdom` environment and includes `ui/**/*.test.{js,jsx}` [vitest.config.js:defineConfig]
- Cargo features of the root crate [Cargo.toml:features]:
  - `oauth-client` pulls in `reqwest`, `base64`, `rand`, and `sha2`
  - `oidc` pulls in `jsonwebtoken` 10.4 (`rust_crypto`), which must match the consumers' version so `DecodingKey` unifies
  - `glitch-scope` pulls in `ipnet`, `sha2`, `thiserror`, `url`, and `uuid`
  - `default = []`
- npm exports [package.json:exports]:
  - `.` maps to `./dist/index.js`
  - `./tokens.css`, `./components.css`, and `./vite` map to the matching `ui/` files
  - `react` and `react-dom` (>=19) are peer dependencies

## Subdirectories & System Boundaries

- src: [./src/EGM.md](./src/EGM.md). The root crate's modules:
  - typed bearer tokens and contract constants
  - cost, feedback, JSON-RPC, and marketplace DTOs
  - SSRF predicates in `net`
  - state-sync rules
  - the security-tester contract
  - UI theme helpers
  - the feature-gated OIDC and Glitch scope compiler
  - `oauth`, documented in [./src/oauth/EGM.md](./src/oauth/EGM.md)
- inference-protocol: [./inference-protocol/EGM.md](./inference-protocol/EGM.md). `dyson-inference-protocol`: `/v1/*` route classification, dialect translation, error envelopes, streaming re-encoding, and `ModelRouter<T>`.
- secrets: [./secrets/EGM.md](./secrets/EGM.md). `dyson-secrets`: redaction and `scrub`, plus the Kingfisher, payment, and external-scanner detectors.
- telegram: [./telegram/EGM.md](./telegram/EGM.md). `dyson-telegram`: the media attachment policy and the bounded Bot API file download.
- traces: [./traces/EGM.md](./traces/EGM.md). `dyson-traces`: Session Traces JSONL, SigV4 signing, and the S3-gateway `Bucket`.
- ui: [./ui/EGM.md](./ui/EGM.md). `dyson-common-ui`: shared React components, hooks, brand marks, theme controller, and Vite build-id plugin.

## Global Invariants

- Contracts shared by `dyson` and `dyson-swarm` are defined once, here. Consumers pin this repository by git tag or revision. The Glitch scope compiler's `SCOPE_COMPILER_VERSION` and golden digest change only as a coordinated release with both consumers [src/glitch_scope.rs:SCOPE_COMPILER_VERSION]
- Detection never validates a found secret with its issuer:
  - Kingfisher is built with its default features only [secrets/Cargo.toml]
  - TruffleHog runs with `--no-verification` [secrets/src/external.rs]
- Inference translation is downstream of usage metering. Billing reads the untranslated upstream bytes [inference-protocol/src/mod.rs:TranslatingStream]
- Errors avoid leaking credentials or URLs:
  - Telegram transport errors omit URLs [telegram/src/file_api.rs:FileDownloadError]
  - OAuth errors are formatted by the caller through `OAuthError::redacted` [src/oauth/client.rs:OAuthError::redacted]
  - upstream inference errors keep only the message [inference-protocol/src/mod.rs:sanitize_upstream_error]
- The UI theme cookie name `dyson-theme` is shared between the Rust and JS halves [src/ui.rs:THEME_COOKIE] [ui/theme.js:createThemeController]

## Global Side Effects

- Network access exists only behind features or in the browser:
  - `oauth-client` [src/oauth/client.rs]
  - `dyson-telegram` with `file-api` [telegram/src/file_api.rs:download_file]
  - `dyson-traces` with `bucket` [traces/src/bucket.rs:Bucket]
  - browser `fetch` calls in `ui/` [ui/buildWatch.js:createBuildWatcher]
- Subprocesses and temp files: the `dyson-secrets` `external` and `kingfisher` features [secrets/src/external.rs:External::run] [secrets/src/kingfisher.rs:generic_rules]
- Browser storage, cookies, clipboard, and global listeners in `ui/` [ui/theme.js:createThemeController]

## System Verification

- Run: `cargo test --workspace` (from `dyson-common/`; add per-crate `--features` as listed in each child contract)
- Run: `npm test` (from `dyson-common/`, UI tests)
