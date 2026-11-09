# dyson-traces (src)

## Overview

Library source for `dyson-traces`. It models agent conversations as Hugging
Face Session Traces: JSON Lines with one `session` header line followed by one
`message` line per message. It also provides AWS SigV4 signing as plain
functions, and (feature `bucket`) an S3-gateway client that publishes session
files. The default endpoint is Hugging Face Storage Buckets. Sessions are plain
serde data, so callers can redact them with `dyson_secrets::scrub` before
publishing.

## Interface

### Sessions (`session.rs`, re-exported from `lib.rs`)

- `Session` [session.rs:Session]
  - `new(harness, id)` [session.rs:Session::new]
  - `named_from_first_user` [session.rs:Session::named_from_first_user]
  - `to_jsonl` [session.rs:Session::to_jsonl]
  - `from_jsonl` [session.rs:Session::from_jsonl]
- `Header` [session.rs:Header]
- `Role`, with `from_wire` [session.rs:Role]
- `Message`, with `new(role, content)` [session.rs:Message]
- `ToolCall` and `Function` [session.rs:ToolCall] [session.rs:Function]
- `ParseError(pub String)` [session.rs:ParseError]
- `object_key(prefix, first_seen, id) -> Option<String>`: produces `<prefix>/YYYY/MM/DD/<id>.jsonl` [session.rs:object_key]

### SigV4 signing (`sigv4.rs`)

- `Credentials` [sigv4.rs:Credentials]
- `payload_sha256` [sigv4.rs:payload_sha256]
- `amz_date` [sigv4.rs:amz_date]
- `authorization` [sigv4.rs:authorization]
- `sign` [sigv4.rs:sign]

### Bucket client (`bucket.rs`, feature `bucket`)

- `Sink` trait [bucket.rs:Sink]
  - `put` and `delete`
  - `get`: the default implementation returns an error
  - `describe`
- `Destination`, with `hugging_face(...)` [bucket.rs:Destination]
- `Bucket::new(&Destination)`, which implements `Sink` [bucket.rs:Bucket]
- `Error(pub String)` [bucket.rs:Error]
- `HF_ENDPOINT` (`https://s3.hf.co`) [bucket.rs:HF_ENDPOINT]
- `HF_REGION` (`us-east-1`) [bucket.rs:HF_REGION]
- `GET_MOST` (32 MiB) [bucket.rs:GET_MOST]

## Invariants

- `object_key` returns `None` if any of these hold [session.rs:object_key]:
  - `id` is empty
  - `id` starts with `.`
  - `id` contains characters other than ASCII alphanumerics and `-_.`
  - the prefix contains characters other than those plus `/`
- Signing assumes path-style requests, object keys that need no percent-encoding, no query string, and a payload hashed as a whole (module docs) [sigv4.rs]
- `Bucket::new` rejects [bucket.rs:Bucket::new]:
  - a non-https endpoint
  - a bucket name with empty, `.`, or `..` path segments, or non-plain characters
- Object keys with empty, `.`, or `..` segments, or with characters that would need encoding, are rejected (private `url` helper) [bucket.rs:Bucket::url]
- `get` fails when an object exceeds `GET_MOST` (32 MiB), whether it is reported by `Content-Length` or counted while streaming [bucket.rs:GET_MOST]
- `delete` of an object that is already gone is not an error (trait contract) [bucket.rs:Sink]
- `named_from_first_user` names the session from the first non-blank line of the first user message. A line longer than `NAME_MOST` (80) chars is clipped, with a trailing marker added [session.rs:Session::named_from_first_user] [session.rs:clipped]

## Side Effects

- `bucket`: SigV4-signed HTTP PUT, DELETE, and GET against the configured S3 gateway, signed with the current time from `Utc::now()` [bucket.rs:Bucket]
- `session` and `sigv4` are pure; time comes from the caller.

## Verification

- Run: `cargo test -p dyson-traces --features bucket` (from `dyson-common/`)
