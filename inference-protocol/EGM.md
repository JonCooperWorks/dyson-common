# inference-protocol

## Overview

The `dyson-inference-protocol` crate, a member of the `dyson-common` workspace.
Its `Cargo.toml` sets the lib path to `src/mod.rs`, and its dependencies are
`bytes`, `futures`, `regex`, `serde_json`, and `tracing`. It holds the
provider-neutral inference API surface:

- route classification
- Anthropic Messages and OpenAI Responses translation to and from OpenAI Chat Completions
- SDK-shaped error envelopes
- streaming response adapters
- a generic deterministic `ModelRouter<T>`

## Subdirectories & Boundaries

- src: [./src/EGM.md](./src/EGM.md). The crate source. It provides:
  - `classify_route` and `dialect_for_path`
  - `Dialect::translate_request`
  - `ResponseTranslator` and `TranslatingStream`
  - `error_body` and `sanitize_upstream_error`
  - `ModelRouter`

  It has no network, disk, or environment access. Hosts must meter usage from the untranslated upstream bytes.

Verification: `cargo test -p dyson-inference-protocol` (from `dyson-common/`).
