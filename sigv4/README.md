# dyson-sigv4

Pure AWS Signature Version 4 (SigV4) signing and presigning for S3 and S3-compatible services.

No HTTP client, no async runtime, and no I/O: callers pass the HTTP method, wire path, query, headers, credentials, service name, and timestamp, and get back an `Authorization` header value or presigned query string.

## What it provides

- `Credentials` (`access_key_id`, `secret_access_key`, `region`) whose `Debug` redacts the secret; `Error` variants carry no input values.
- `canonical_uri(path)`: canonicalizes a path **as sent on the wire**.
- `canonical_query(raw_query)` and `canonical_query_pairs(pairs)`: canonicalizes raw wire query strings or already-decoded `(name, value)` pairs.
- `authorization(...)` and `sign(...)`: header signing supporting hashed payloads (`payload_sha256`) and `UNSIGNED_PAYLOAD`, arbitrary signed header sets (requiring `host`), and configurable service names (`S3_SERVICE = "s3"`).
- `presign_query(...)`, `presign_query_with_headers(...)`, and `presign_url(...)`: query-parameter presigning using `UNSIGNED-PAYLOAD` with expiry enforced to `1..=604_800` seconds.
- `strip_presign_params(raw_query)`: removes all seven SigV4 presign parameters (`X-Amz-Algorithm`, `X-Amz-Credential`, `X-Amz-Date`, `X-Amz-Expires`, `X-Amz-SignedHeaders`, `X-Amz-Signature`, `X-Amz-Security-Token`, matched case-insensitively on the decoded name) while leaving all other parameters byte-for-byte intact and in their original order.

## Encoding and canonicalization rules

- **Path as sent, no normalization:** each `/`-separated segment is percent-decoded once and re-encoded once with the RFC 3986 unreserved set (`A-Z a-z 0-9 - . _ ~`, uppercase hex). S3 does not collapse `//`, `.`, or `..`, so neither does this crate. An empty path becomes `/`, and a path without a leading `/` gets one prepended.
- **`+` is literal:** `+` is never decoded to a space in either paths or query strings; it canonicalizes to `%2B`.
- **Invalid percent escapes:** a `%` not followed by two ASCII hex digits (e.g. `%zz`, `%2`, trailing `%`) is treated as a literal `%` byte and encoded as `%25`.
- **Bare query parameters:** a parameter without `=` (e.g. `?uploads`, `?lifecycle`) canonicalizes to `name=`. Empty segments between stray `&` separators are skipped.
- **Header normalization:** header names are trimmed of ASCII spaces and lowercased; header values have leading/trailing ASCII spaces (`' '`) stripped and inner runs of ASCII spaces collapsed to a single space; duplicate header names are joined with `,` in input order.
