//! Agent conversations as Hugging Face Session Traces, and a bucket to
//! publish them to.
//!
//! * [`Session`]: one conversation as a JSON Lines file the Hugging Face
//!   trace viewer reads, a `session` header line and a `message` line a
//!   message ([`Session::to_jsonl`], [`Session::from_jsonl`]).
//! * [`object_key`]: where a session's file is kept, by the day it began.
//! * [`sigv4`]: AWS Signature Version 4 for S3, as plain functions.
//! * `bucket` feature: a [`Sink`](bucket::Sink) trait and an S3-gateway
//!   [`Bucket`](bucket::Bucket) client, defaulting to Hugging Face's
//!   Storage Buckets (`https://s3.hf.co`).
//!
//! Redact before you publish: a [`Session`] is plain serde data, so it can
//! be scrubbed as a JSON document with `dyson_secrets::scrub`.

pub mod session;
pub mod sigv4;

#[cfg(feature = "bucket")]
pub mod bucket;

pub use session::{Function, Header, Message, ParseError, Role, Session, ToolCall, object_key};
