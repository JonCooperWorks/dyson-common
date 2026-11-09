# traces

## Overview

The `dyson-traces` crate, a member of the `dyson-common` workspace. It models
agent conversations as Hugging Face Session Traces (JSONL), and with the
`bucket` feature (which pulls in `async-trait` and `reqwest`) it can publish
them to an S3-gateway bucket.

## Subdirectories & Boundaries

- src: [./src/EGM.md](./src/EGM.md). The crate source. It provides:
  - `Session` with `to_jsonl` and `from_jsonl`
  - `object_key`
  - `sigv4` signing functions
  - `Sink` and `Bucket`, with the `bucket` feature. The default endpoint is `https://s3.hf.co`, only https is allowed, and reads are capped at 32 MiB.

Verification: `cargo test -p dyson-traces --features bucket` (from `dyson-common/`).
