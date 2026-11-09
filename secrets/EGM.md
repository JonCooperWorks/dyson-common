# secrets

## Overview

The `dyson-secrets` crate, a member of the `dyson-common` workspace. It
detects and redacts secrets in agent traffic and traces.

Redaction is always built. The detectors are behind features:

- `kingfisher`
- `payment`
- `external`

A further `utoipa` feature only adds OpenAPI schema derives.

Next to `src/` there are two data directories, which are not code:

- `data/`: the compressed BIN table (`bins.tsv.gz`, described in `data/README.md`), embedded by the payment detector
- `rules/`: `betterleaks-generic.toml` and `trufflehog-exclude.txt`, embedded by the Kingfisher and external detectors

## Subdirectories & Boundaries

- src: [./src/EGM.md](./src/EGM.md). The crate source. It provides:
  - `Redactor`, `scrub`, and the `[REDACTED:<rule>:<digest>]` tokens
  - the `Hit` finding types
  - the Kingfisher `Scanner`
  - the payment `find` (card numbers and IBANs)
  - `External` subprocess scanners
- data: the BIN table, consumed via `include_bytes!` in `src/payment.rs`. No contract file.
- rules: detector rule files, consumed via `include_str!` in `src/kingfisher.rs` and `src/external.rs`. No contract file.

Verification: `cargo test -p dyson-secrets --features payment,external` (from `dyson-common/`).
