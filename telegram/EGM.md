# telegram

## Overview

The `dyson-telegram` crate, a member of the `dyson-common` workspace. It holds
the Telegram attachment policy shared by Dyson runtimes, and a bounded file
download behind the `file-api` feature (which pulls in `reqwest` and
`futures-util`). The root `dyson-common` crate depends on it with default
features off and re-exports `media` as `dyson_common::telegram_media`.

## Subdirectories & Boundaries

- src: [./src/EGM.md](./src/EGM.md). The crate source. It provides:
  - `media`: `DownloadLimits`, `DocumentKind`, `classify_document`, `effective_mime`, and the MIME and extension predicates
  - `download_file` and `FileDownloadError`, with the `file-api` feature

  `download_file` performs a bounded, URL-free-error `getFile` call followed by the download.

Verification: `cargo test -p dyson-telegram --features file-api` (from `dyson-common/`).
