# dyson-telegram (src)

## Overview

Library source for `dyson-telegram`. It holds the Telegram attachment policy
shared by Dyson runtimes, and a bounded Bot API file download behind the
`file-api` feature. `media` is pure policy (document classification, effective
MIME type, size limits). `file_api` performs the two-step
`getFile`-then-download transfer.

## Interface

### Media policy (`media.rs`)

- `DownloadLimits` [media.rs:DownloadLimits]
  - fields: `image_max_bytes`, `audio_max_bytes`, `document_max_bytes`, `text_max_bytes`
  - `for_document(kind)`: the limit for a document kind [media.rs:DownloadLimits::for_document]
- `DocumentKind`: `Image`, `Pdf`, `Office`, `Text`, `Binary` [media.rs:DocumentKind]
- `classify_document(mime, file_name)` [media.rs:classify_document]
- `effective_mime(original, kind, file_name)` [media.rs:effective_mime]
- `extension_of(name)` [media.rs:extension_of]
- MIME predicates: `is_text_like_mime`, `is_office_mime` [media.rs:is_text_like_mime] [media.rs:is_office_mime]
- Extension predicates: `is_office_extension`, `is_image_extension`, `is_text_extension` [media.rs:is_office_extension] [media.rs:is_image_extension] [media.rs:is_text_extension]

### File download (`file_api.rs`, feature `file-api`, re-exported from `lib.rs`)

- `download_file(client, method_base_url, file_base_url, bearer, file_id, max_bytes) -> Result<Vec<u8>, FileDownloadError>` [file_api.rs:download_file]
- `FileDownloadError` [file_api.rs:FileDownloadError]

## Invariants

- Default limits [media.rs:DownloadLimits::default]:
  - images: 50 MiB
  - audio: 50 MiB
  - documents: 200 MiB
  - text: 1 MiB
- `for_document` applies the text limit to `Text`, the image limit to `Image`, and the document limit to `Pdf`, `Office`, and `Binary` [media.rs:DownloadLimits::for_document]
- `download_file` enforces `max_bytes` three times [file_api.rs:download_file]:
  - against the reported `file_size`
  - against the `Content-Length` header
  - against the running total while streaming
- `download_file` errors [file_api.rs:download_file]:
  - `ok: false` in the response envelope gives `Telegram`
  - a missing or empty `file_path` gives `MissingPath`
- Transport errors carry only a stage name, never a URL, so a bot token embedded in a direct Bot API URL cannot leak through an error string [file_api.rs:FileDownloadError] [file_api.rs:download_file]

## Side Effects

- `file-api` makes two HTTP requests with the caller's `reqwest::Client` [file_api.rs:download_file]:
  - POST `<method_base_url>/getFile` with `{"file_id": ...}`
  - GET `<file_base_url>/<file_path>`
  - both send `Authorization: Bearer` if `bearer` is set
- `media` has no side effects.

## Verification

- Run: `cargo test -p dyson-telegram --features file-api` (from `dyson-common/`)
