# dyson-secrets (src)

## Overview

Library source for `dyson-secrets`: secret detection and redaction for agent
traffic, logs, and traces. Redaction (`redact`, `hit`, `text`) is always built.

The detectors are behind features:

- `kingfisher`: MongoDB Kingfisher plus Betterleaks' generic rules
- `payment`: card numbers and IBANs, with a compiled-in BIN table
- `external`: Betterleaks, TruffleHog, and Nosey Parker run as subprocesses

A further `utoipa` feature only adds OpenAPI schema derives. Detection only:
nothing here ever presents a found key to its issuer.

## Interface

### Crate root

- `Error(pub String)`: the crate's error type [lib.rs:Error]
- Re-exports `Hit`, `ContextLine`, `Detection`, `Extra`, `Issuer`, `KingfisherLocation`, `Dependency`, `Paired`, and `pair` [lib.rs]
- Re-exports `Redactor`, `Leftover`, `Scrubbed`, and `scrub` [lib.rs]
- With the `kingfisher` feature, also re-exports `Scanner` and `At` [lib.rs]

### Findings (`hit.rs`)

- Finding types `Hit`, `ContextLine`, `Detection`, `KingfisherLocation`, `Dependency`, `Paired`, `Issuer`, and `Extra`. With the `utoipa` feature they derive `utoipa::ToSchema` [hit.rs:Hit] [hit.rs:Extra]
- `pair(hits)`: links related hits [hit.rs:pair]

### Text helpers (`text.rs`)

- `LINE_CAP` (200) [text.rs:LINE_CAP]
- `entropy`, `unescape`, `short_name`, `same_secret`, `line_col`, `assigned_name`, `clip`, `surroundings`, `mask` [text.rs:entropy] [text.rs:mask]

### Redaction (`redact.rs`)

- `token(rule_id, digest)`: produces `[REDACTED:<rule>:<8 hex>]` [redact.rs:token]
- `label_token(label)`: produces `[REDACTED:<label>]` [redact.rs:label_token]
- `digest(key, value)`: HMAC-SHA256 as lowercase hex [redact.rs:digest]
- `distinctive`, `needles`, `strings`, `tokens_in`, `is_token`, and `SHORTEST` (4) [redact.rs:distinctive] [redact.rs:needles] [redact.rs:SHORTEST]
- `RedactorBuilder` with `secret`, `any_case`, and `build` [redact.rs:RedactorBuilder]
- `Redactor` with `builder`, `is_empty`, `text`, and `document` [redact.rs:Redactor]
- `Leftover { value, rule_id, token, shown }` [redact.rs:Leftover]
- `Scrubbed::{Clean { redactions }, Held(String)}` [redact.rs:Scrubbed]
- `scrub(document, redactor, rounds, scan)`: redact, rescan, and repeat [redact.rs:scrub]

### Kingfisher scanner (`kingfisher.rs`, feature `kingfisher`)

- `Scanner` with `new`, `shared`, `scan`, `scan_at`, and `leftovers(document, key, keep)`, which yields the `Leftover`s for a `scrub` rescan [kingfisher.rs:Scanner] [kingfisher.rs:Scanner::leftovers]
- `At` builder (`new`, `path`, `prefix`, `first_line`, `payment`) that describes where a scanned text sits [kingfisher.rs:At]

### Payment details (`payment.rs`, feature `payment`)

- `Found` [payment.rs:Found]
- `issuer(digits)`: BIN lookup [payment.rs:issuer]
- `rule(id)`: true when a rule id belongs to this module (`iban`, or one starting with `card.`) [payment.rs:rule]
- `find(text)`: card-number and IBAN finder [payment.rs:find]

### External scanners (`external.rs`, feature `external`)

- `External` with `new(dir)`, `exclude`, `skip_value`, `versions`, and `run(texts) -> Vec<Vec<Extra>>` [external.rs:External]
- `SHORTEST` (8) [external.rs:SHORTEST]

## Invariants

- No detector validates against an issuer:
  - Only Kingfisher's default features are built [kingfisher.rs]
  - TruffleHog runs as `filesystem --no-verification` [external.rs]
- `scrub` behaviour [redact.rs:scrub]:
  - It redacts with the known secrets, then calls the caller's `scan` up to `rounds` extra times.
  - The result is `Held` if a secret is still present after the last round.
  - The result is also `Held` if a shown secret is found that appears in no string verbatim.
  - A scan error is propagated, so the document must not be passed on.
- Rescan findings shorter than `SHORTEST` (4 chars), or already redaction tokens, are ignored [redact.rs:scrub]
- A value is removed from every document only if it is `distinctive`: at least 8 chars, and either at least 20 chars or entropy of at least 3.0 bits per char [redact.rs:distinctive]
- Rule names in tokens are filtered to `[A-Za-z0-9._-]` and capped at 48 chars, falling back to `secret`. Digests are truncated to 8 hex chars [redact.rs:token] [redact.rs:TOKEN]
- External scanners run with a cleared environment: only `HOME`, `XDG_CACHE_HOME`, `XDG_CONFIG_HOME`, `PATH=/usr/local/bin:/usr/bin:/bin`, and `NO_COLOR=1` are set, and stdin is null. A missing or failing program is an error, never "no findings" [external.rs:External::command] [external.rs:External::versions]
- The external scratch directory is set to mode `0o700` on Unix and removed when dropped [external.rs:Scratch]
- Card candidates match `\d(?:[ -]?\d){11,}`. IBAN candidates match `[A-Z]{2}\d{2} ?[A-Z0-9]{4}` and must be 15 to 34 chars long [payment.rs:RUN] [payment.rs:IBAN] [payment.rs:IBAN_SHORTEST] [payment.rs:IBAN_LONGEST]
- `Scanner::shared` builds the scanner once per process, because compiling the rules takes seconds [kingfisher.rs:SHARED]

## Side Effects

- `external`: spawns the `betterleaks`, `trufflehog`, and `noseyparker` binaries from the configured `dir`. Writes each batch of texts to a scratch directory under `std::env::temp_dir()` and removes it afterwards [external.rs:External::run] [external.rs:Scratch::new]
- `kingfisher`: writes the embedded Betterleaks generic rules to `temp_dir()/dyson-secrets-rules-<pid>.toml`, loads them, and deletes the file [kingfisher.rs:generic_rules]
- Process-wide lazily initialised statics:
  - the shared scanner [kingfisher.rs:SHARED]
  - the decompressed BIN table, from `include_bytes!("../data/bins.tsv.gz")` [payment.rs:BINS]
  - compiled regexes [payment.rs:RUN] [redact.rs:TOKEN]
- Embedded at compile time:
  - `rules/trufflehog-exclude.txt` [external.rs:TRUFFLEHOG_EXCLUDE]
  - `rules/betterleaks-generic.toml` [kingfisher.rs:GENERIC_RULES]
- No network calls.

## Verification

- Run: `cargo test -p dyson-secrets --features payment,external` (from `dyson-common/`). Add `kingfisher` to also test the Kingfisher scanner; compiling its rules takes minutes unoptimised.
