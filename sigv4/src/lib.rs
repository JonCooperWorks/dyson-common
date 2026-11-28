//! Pure AWS Signature Version 4 (SigV4) signing and presigning.
//!
//! No HTTP client, no async runtime, and no I/O: callers pass the HTTP method,
//! wire path, query, headers, credentials, service name, and timestamp, and get
//! back an `Authorization` header value or presigned query string.
//!
//! ## Encoding and canonicalization rules
//!
//! This crate follows Amazon S3's SigV4 rules:
//!
//! - **Path as sent, no normalization:** [`canonical_uri`] accepts the path as
//!   sent on the wire (already percent-encoded or not). Each `/`-separated
//!   segment is percent-decoded once and then encoded once with the RFC 3986
//!   unreserved set (`A-Z a-z 0-9 - . _ ~`, uppercase hex). S3 does **not**
//!   collapse `//`, `.`, or `..`; neither does this crate. An empty path
//!   becomes `"/"`, and a path without a leading `'/'` gets one prepended.
//! - **`+` is always literal:** neither paths nor query strings treat `+` as a
//!   space. A literal `+` canonicalizes to `%2B`.
//! - **Invalid percent escapes:** a `%` not followed by two ASCII hex digits
//!   (such as `%zz`, `%2`, or a trailing `%`) is treated as a literal `%` byte
//!   and encoded as `%25`.
//! - **Query strings:** [`canonical_query`] takes a raw query string (without a
//!   leading `'?'`), splits on `'&'` (ignoring empty segments), splits each
//!   item on the first `'='`, percent-decodes name and value (keeping `+`
//!   literal), encodes each with the RFC 3986 unreserved set, sorts by encoded
//!   name then encoded value, and joins as `name=value` (a bare parameter such
//!   as `uploads` becomes `uploads=`).
//! - **Headers:** header names are trimmed of ASCII spaces and lowercased;
//!   values have leading/trailing ASCII spaces (`' '`) stripped and inner runs
//!   of ASCII spaces collapsed to a single space. Duplicate header names are
//!   joined with `','` in input order. `host` is always required.
//! - **Secrets never leak:** [`Credentials`]'s [`fmt::Debug`] redacts
//!   `secret_access_key`, and [`Error`] carries no input values.

use std::collections::BTreeMap;
use std::fmt;

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac as _};
use sha2::{Digest as _, Sha256};

/// Default AWS service name for Amazon S3 and S3-compatible object stores.
pub const S3_SERVICE: &str = "s3";

/// Payload hash literal for requests whose body is not hashed (e.g. egress
/// proxy re-signing or S3 presigned URLs).
pub const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";

/// Maximum presigned URL validity allowed by AWS SigV4 (7 days, in seconds).
pub const MAX_EXPIRES_SECS: u64 = 604_800;

const ALGORITHM: &str = "AWS4-HMAC-SHA256";
const TERMINATOR: &str = "aws4_request";
const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";

const PRESIGN_PARAMS: &[&[u8]] = &[
    b"x-amz-algorithm",
    b"x-amz-credential",
    b"x-amz-date",
    b"x-amz-expires",
    b"x-amz-signedheaders",
    b"x-amz-signature",
    b"x-amz-security-token",
];

/// AWS signing credentials and region.
///
/// [`fmt::Debug`] never shows `secret_access_key`.
#[derive(Clone)]
pub struct Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
}

impl Credentials {
    /// Constructs credentials from an access key ID, secret access key, and region.
    pub fn new(
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
        region: impl Into<String>,
    ) -> Self {
        Self {
            access_key_id: access_key_id.into(),
            secret_access_key: secret_access_key.into(),
            region: region.into(),
        }
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"…")
            .field("region", &self.region)
            .finish()
    }
}

/// Errors returned when signing inputs are invalid.
///
/// Variants carry no input data, so secrets and request contents can never
/// appear in `Debug` or `Display` output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The `host` header was omitted or blank.
    MissingHost,
    /// A presigned URL expiry was `0` or exceeded [`MAX_EXPIRES_SECS`] (604800 seconds).
    InvalidExpiry,
    /// A header name was empty or contained invalid characters.
    InvalidHeaderName,
    /// A header value contained CR or LF.
    InvalidHeaderValue,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHost => f.write_str("missing or empty host header"),
            Self::InvalidExpiry => {
                f.write_str("presigned URL expiry must be between 1 and 604800 seconds")
            }
            Self::InvalidHeaderName => f.write_str("invalid header name"),
            Self::InvalidHeaderValue => f.write_str("header value contains CR or LF"),
        }
    }
}

impl std::error::Error for Error {}

/// Lowercase hex SHA-256 digest of a payload, as carried by `x-amz-content-sha256`.
pub fn payload_sha256(body: &[u8]) -> String {
    hex::encode(Sha256::digest(body))
}

/// Formats a UTC timestamp in SigV4 basic ISO 8601 form (`YYYYMMDDTHHMMSSZ`).
pub fn amz_date(now: DateTime<Utc>) -> String {
    now.format("%Y%m%dT%H%M%SZ").to_string()
}

/// Produces the S3 canonical URI from a path as sent on the wire.
///
/// Each `/`-delimited segment is percent-decoded once (keeping `+` literal and
/// treating invalid `%` escapes as a literal `%`) and then percent-encoded once
/// using the RFC 3986 unreserved set (`A-Z a-z 0-9 - . _ ~`). `/` separators
/// are preserved with no path normalization (`//`, `.`, `..` are not collapsed).
/// An empty path becomes `"/"`, and a relative path gets a leading `'/'`.
pub fn canonical_uri(path: &str) -> String {
    if path.is_empty() {
        return "/".to_owned();
    }
    let mut out = String::with_capacity(path.len() + 1);
    let rest = path.strip_prefix('/').unwrap_or(path);
    out.push('/');
    for (idx, segment) in rest.split('/').enumerate() {
        if idx > 0 {
            out.push('/');
        }
        let decoded = percent_decode(segment.as_bytes());
        out.push_str(&percent_encode(&decoded));
    }
    out
}

/// Produces the SigV4 canonical query string from a raw wire query string
/// (without a leading `'?'`).
///
/// Splits on `'&'` (ignoring empty segments), splits each parameter on the
/// first `'='`, percent-decodes name and value (keeping `+` literal and
/// treating invalid `%` escapes as literal `%`), encodes each with the RFC 3986
/// unreserved set, sorts by encoded name then encoded value, and joins as
/// `name=value` (bare names such as `uploads` become `uploads=`).
pub fn canonical_query(raw_query: &str) -> String {
    if raw_query.is_empty() {
        return String::new();
    }
    let mut encoded: Vec<(String, String)> = raw_query
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (raw_name, raw_value) = part.split_once('=').unwrap_or((part, ""));
            (
                percent_encode(&percent_decode(raw_name.as_bytes())),
                percent_encode(&percent_decode(raw_value.as_bytes())),
            )
        })
        .collect();
    format_sorted_query(&mut encoded)
}

/// Produces the SigV4 canonical query string from already-decoded `(name, value)` pairs.
///
/// Each name and value is percent-encoded with the RFC 3986 unreserved set,
/// sorted by encoded name then encoded value, and joined as `name=value`.
pub fn canonical_query_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut encoded: Vec<(String, String)> = pairs
        .into_iter()
        .map(|(name, value)| {
            (
                percent_encode(name.as_bytes()),
                percent_encode(value.as_bytes()),
            )
        })
        .collect();
    format_sorted_query(&mut encoded)
}

/// Removes all SigV4 presign parameters from a raw query string, preserving all
/// other parameters byte-for-byte and in their original order.
///
/// Parameter names are matched case-insensitively after percent-decoding the
/// name before the first `'='`. The stripped parameters are:
/// `X-Amz-Algorithm`, `X-Amz-Credential`, `X-Amz-Date`, `X-Amz-Expires`,
/// `X-Amz-SignedHeaders`, `X-Amz-Signature`, and `X-Amz-Security-Token`.
pub fn strip_presign_params(raw_query: &str) -> String {
    if raw_query.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(raw_query.len());
    let mut first = true;
    for part in raw_query.split('&') {
        let raw_name = part.split_once('=').map_or(part, |(name, _)| name);
        let decoded = percent_decode(raw_name.as_bytes());
        let is_presign = PRESIGN_PARAMS
            .iter()
            .any(|&target| decoded.eq_ignore_ascii_case(target));
        if !is_presign {
            if !first {
                out.push('&');
            }
            first = false;
            out.push_str(part);
        }
    }
    out
}

/// Computes the SigV4 `Authorization` header value for a request.
///
/// - `path` is the path as sent on the wire (canonicalized by [`canonical_uri`]).
/// - `raw_query` is the raw query string without a leading `'?'` (canonicalized
///   by [`canonical_query`]).
/// - `headers` is the list of headers to sign; `host` must be present and
///   non-empty or [`Error::MissingHost`] is returned.
/// - `payload_hash` is either a hex SHA-256 digest (from [`payload_sha256`]) or
///   [`UNSIGNED_PAYLOAD`].
#[allow(clippy::too_many_arguments)]
pub fn authorization(
    method: &str,
    path: &str,
    raw_query: &str,
    headers: &[(&str, &str)],
    payload_hash: &str,
    credentials: &Credentials,
    service: &str,
    now: DateTime<Utc>,
) -> Result<String, Error> {
    let (canonical_headers, signed_headers) = canonicalize_headers(headers)?;
    let canon_uri = canonical_uri(path);
    let canon_query = canonical_query(raw_query);

    let canonical_request = format!(
        "{method}\n{canon_uri}\n{canon_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );
    let date_stamp = now.format("%Y%m%d").to_string();
    let stamp = amz_date(now);
    let scope = credential_scope(&date_stamp, &credentials.region, service);
    let signature = sign_canonical_request(
        &canonical_request,
        &stamp,
        &date_stamp,
        &scope,
        credentials,
        service,
    );

    Ok(format!(
        "{ALGORITHM} Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
        credentials.access_key_id
    ))
}

/// Convenience wrapper that signs a request with `host`, `x-amz-content-sha256`,
/// and `x-amz-date`, returning the three headers to attach:
/// `x-amz-content-sha256`, `x-amz-date`, and `authorization`.
///
/// `payload_hash` may be a hex SHA-256 digest from [`payload_sha256`] or
/// [`UNSIGNED_PAYLOAD`].
#[allow(clippy::too_many_arguments)]
pub fn sign(
    method: &str,
    host: &str,
    path: &str,
    raw_query: &str,
    payload_hash: &str,
    credentials: &Credentials,
    service: &str,
    now: DateTime<Utc>,
) -> Result<[(&'static str, String); 3], Error> {
    let stamp = amz_date(now);
    let auth = authorization(
        method,
        path,
        raw_query,
        &[
            ("host", host),
            ("x-amz-content-sha256", payload_hash),
            ("x-amz-date", &stamp),
        ],
        payload_hash,
        credentials,
        service,
        now,
    )?;
    Ok([
        ("x-amz-content-sha256", payload_hash.to_owned()),
        ("x-amz-date", stamp),
        ("authorization", auth),
    ])
}

/// Produces the full canonical presigned query string to append after `'?'`,
/// signing the `host` header and using [`UNSIGNED_PAYLOAD`].
///
/// `extra_query` holds already-decoded `(name, value)` parameters (for example
/// `partNumber`, `uploadId`, `versionId`, `response-content-type`,
/// `response-content-disposition`). `expires_secs` must be in `1..=604_800`
/// (7 days).
#[allow(clippy::too_many_arguments)]
pub fn presign_query(
    method: &str,
    host: &str,
    path: &str,
    extra_query: &[(&str, &str)],
    expires_secs: u64,
    credentials: &Credentials,
    service: &str,
    now: DateTime<Utc>,
) -> Result<String, Error> {
    presign_query_with_headers(
        method,
        path,
        extra_query,
        &[("host", host)],
        expires_secs,
        credentials,
        service,
        now,
    )
}

/// Like [`presign_query`], but allows signing additional headers alongside
/// `host`. `headers` must include a non-empty `host` header.
#[allow(clippy::too_many_arguments)]
pub fn presign_query_with_headers(
    method: &str,
    path: &str,
    extra_query: &[(&str, &str)],
    headers: &[(&str, &str)],
    expires_secs: u64,
    credentials: &Credentials,
    service: &str,
    now: DateTime<Utc>,
) -> Result<String, Error> {
    if expires_secs == 0 || expires_secs > MAX_EXPIRES_SECS {
        return Err(Error::InvalidExpiry);
    }
    let (canonical_headers, signed_headers) = canonicalize_headers(headers)?;
    let canon_uri = canonical_uri(path);

    let date_stamp = now.format("%Y%m%d").to_string();
    let stamp = amz_date(now);
    let scope = credential_scope(&date_stamp, &credentials.region, service);
    let credential = format!("{}/{scope}", credentials.access_key_id);
    let expires = expires_secs.to_string();

    let mut params: Vec<(&str, &str)> = Vec::with_capacity(extra_query.len() + 5);
    params.push(("X-Amz-Algorithm", ALGORITHM));
    params.push(("X-Amz-Credential", &credential));
    params.push(("X-Amz-Date", &stamp));
    params.push(("X-Amz-Expires", &expires));
    params.push(("X-Amz-SignedHeaders", &signed_headers));
    params.extend_from_slice(extra_query);

    let canon_query = canonical_query_pairs(params);
    let canonical_request = format!(
        "{method}\n{canon_uri}\n{canon_query}\n{canonical_headers}\n{signed_headers}\n{UNSIGNED_PAYLOAD}"
    );
    let signature = sign_canonical_request(
        &canonical_request,
        &stamp,
        &date_stamp,
        &scope,
        credentials,
        service,
    );

    Ok(format!("{canon_query}&X-Amz-Signature={signature}"))
}

/// Builds a complete presigned URL string (`{scheme}://{host}{canonical_path}?{query}`).
#[allow(clippy::too_many_arguments)]
pub fn presign_url(
    scheme: &str,
    method: &str,
    host: &str,
    path: &str,
    extra_query: &[(&str, &str)],
    expires_secs: u64,
    credentials: &Credentials,
    service: &str,
    now: DateTime<Utc>,
) -> Result<String, Error> {
    let query = presign_query(
        method,
        host,
        path,
        extra_query,
        expires_secs,
        credentials,
        service,
        now,
    )?;
    let clean_host = host.trim_matches(' ');
    let canon_path = canonical_uri(path);
    Ok(format!("{scheme}://{clean_host}{canon_path}?{query}"))
}

const fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
}

const fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'A'..=b'F' => Some(b - b'A' + 10),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

fn percent_decode(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'%'
            && i + 2 < input.len()
            && let (Some(hi), Some(lo)) = (hex_nibble(input[i + 1]), hex_nibble(input[i + 2]))
        {
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        out.push(input[i]);
        i += 1;
    }
    out
}

fn percent_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        if is_unreserved(b) {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX_UPPER[(b >> 4) as usize] as char);
            out.push(HEX_UPPER[(b & 0x0f) as usize] as char);
        }
    }
    out
}

fn format_sorted_query(encoded: &mut [(String, String)]) -> String {
    encoded.sort();
    let mut out = String::new();
    for (idx, (name, value)) in encoded.iter().enumerate() {
        if idx > 0 {
            out.push('&');
        }
        out.push_str(name);
        out.push('=');
        out.push_str(value);
    }
    out
}

fn normalize_header_value(raw: &str) -> Result<String, Error> {
    if raw.contains(['\r', '\n']) {
        return Err(Error::InvalidHeaderValue);
    }
    let trimmed = raw.trim_matches(' ');
    let mut out = String::with_capacity(trimmed.len());
    let mut prev_space = false;
    for ch in trimmed.chars() {
        if ch == ' ' {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    Ok(out)
}

fn canonicalize_headers(headers: &[(&str, &str)]) -> Result<(String, String), Error> {
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for &(raw_name, raw_value) in headers {
        let name = raw_name.trim_matches(' ').to_ascii_lowercase();
        if name.is_empty()
            || name
                .bytes()
                .any(|b| b <= 0x20 || b >= 0x7f || b == b':' || b == b';')
        {
            return Err(Error::InvalidHeaderName);
        }
        let value = normalize_header_value(raw_value)?;
        grouped.entry(name).or_default().push(value);
    }

    match grouped.get("host") {
        Some(vals) if !vals.is_empty() && vals.iter().all(|v| !v.is_empty()) => {}
        _ => return Err(Error::MissingHost),
    }

    let mut canonical = String::new();
    let mut signed = String::new();
    for (idx, (name, values)) in grouped.iter().enumerate() {
        if idx > 0 {
            signed.push(';');
        }
        signed.push_str(name);
        canonical.push_str(name);
        canonical.push(':');
        for (v_idx, val) in values.iter().enumerate() {
            if v_idx > 0 {
                canonical.push(',');
            }
            canonical.push_str(val);
        }
        canonical.push('\n');
    }

    Ok((canonical, signed))
}

fn credential_scope(date_stamp: &str, region: &str, service: &str) -> String {
    format!("{date_stamp}/{region}/{service}/{TERMINATOR}")
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC-SHA256 accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

fn sign_canonical_request(
    canonical_request: &str,
    stamp: &str,
    date_stamp: &str,
    scope: &str,
    credentials: &Credentials,
    service: &str,
) -> String {
    let hashed_request = hex::encode(Sha256::digest(canonical_request.as_bytes()));
    let string_to_sign = format!("{ALGORITHM}\n{stamp}\n{scope}\n{hashed_request}");

    let mut initial_key = Vec::with_capacity(4 + credentials.secret_access_key.len());
    initial_key.extend_from_slice(b"AWS4");
    initial_key.extend_from_slice(credentials.secret_access_key.as_bytes());

    let k_date = hmac_sha256(&initial_key, date_stamp.as_bytes());
    let k_region = hmac_sha256(&k_date, credentials.region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    let k_signing = hmac_sha256(&k_service, TERMINATOR.as_bytes());

    hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(rfc3339: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(rfc3339)
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn canonical_uri_handles_awkward_inputs() {
        assert_eq!(canonical_uri(""), "/");
        assert_eq!(canonical_uri("/"), "/");
        assert_eq!(canonical_uri("//"), "//");
        assert_eq!(canonical_uri("/a//b"), "/a//b");
        assert_eq!(canonical_uri("/a/./b/../c"), "/a/./b/../c");
        assert_eq!(canonical_uri("/trailing/"), "/trailing/");
        assert_eq!(canonical_uri("test$file.text"), "/test%24file.text");

        // Literal `+` encodes to `%2B`; already-encoded `%2B` stays `%2B`.
        assert_eq!(canonical_uri("/a+b"), "/a%2Bb");
        assert_eq!(canonical_uri("/a%2Bb"), "/a%2Bb");
        assert_eq!(canonical_uri("/a%2bb"), "/a%2Bb");

        // `%2F` inside a segment stays `%2F` (and normalizes lowercase hex).
        assert_eq!(canonical_uri("/bucket/a%2Fb"), "/bucket/a%2Fb");
        assert_eq!(canonical_uri("/bucket/a%2fb"), "/bucket/a%2Fb");
        assert_eq!(canonical_uri("/bucket/a%252Fb"), "/bucket/a%252Fb");

        // Spaces, raw and encoded.
        assert_eq!(canonical_uri("/hello world.txt"), "/hello%20world.txt");
        assert_eq!(canonical_uri("/hello%20world.txt"), "/hello%20world.txt");

        // Non-ASCII UTF-8, raw and encoded.
        assert_eq!(canonical_uri("/ሴ"), "/%E1%88%B4");
        assert_eq!(canonical_uri("/%e1%88%b4"), "/%E1%88%B4");
        assert_eq!(
            canonical_uri("/café/日本語.txt"),
            "/caf%C3%A9/%E6%97%A5%E6%9C%AC%E8%AA%9E.txt"
        );

        // Unreserved characters are decoded out of percent-encoding.
        assert_eq!(canonical_uri("/%7Euser"), "/~user");
        assert_eq!(
            canonical_uri("/-._~0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"),
            "/-._~0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
        );

        // Invalid or truncated percent escapes: `%` treated as literal `%` -> `%25`.
        assert_eq!(canonical_uri("/a%zzb"), "/a%25zzb");
        assert_eq!(canonical_uri("/a%2"), "/a%252");
        assert_eq!(canonical_uri("/a%"), "/a%25");
        assert_eq!(canonical_uri("/a%2%20b"), "/a%252%20b");
        assert_eq!(canonical_uri("/a%2Gb"), "/a%252Gb");

        // Idempotency: canonicalizing an already-canonical URI is a no-op.
        for p in [
            "",
            "/",
            "//",
            "/a//b",
            "/a/./b/../c",
            "/a+b",
            "/bucket/a%2Fb",
            "/bucket/a%252Fb",
            "/hello world.txt",
            "/ሴ",
            "/%7Euser",
            "/a%zzb",
            "/a%2%20b",
        ] {
            let once = canonical_uri(p);
            assert_eq!(canonical_uri(&once), once, "not idempotent for {p:?}");
        }
    }

    #[test]
    fn canonical_query_handles_encoding_sorting_and_edge_cases() {
        assert_eq!(canonical_query(""), "");
        assert_eq!(canonical_query("uploads"), "uploads=");
        assert_eq!(canonical_query("lifecycle"), "lifecycle=");
        assert_eq!(
            canonical_query("partNumber=2&uploadId=abc%2Fdef"),
            "partNumber=2&uploadId=abc%2Fdef"
        );
        // Literal `+` in query MUST NOT become space; it canonicalizes to `%2B`.
        assert_eq!(canonical_query("k=a+b"), "k=a%2Bb");
        assert_eq!(canonical_query("a+b=c+d"), "a%2Bb=c%2Bd");
        assert_eq!(canonical_query("k=a%20b"), "k=a%20b");

        // Continuation token with `+`, `/`, `=`, spaces, UTF-8, and invalid escapes.
        assert_eq!(
            canonical_query("list-type=2&prefix=a%2Fb&continuation-token=abc+def/ghi=="),
            "continuation-token=abc%2Bdef%2Fghi%3D%3D&list-type=2&prefix=a%2Fb"
        );
        assert_eq!(
            canonical_query("list-type=2&prefix=a/b&continuation-token=abc%2Bdef%2Fghi%3D%3D"),
            "continuation-token=abc%2Bdef%2Fghi%3D%3D&list-type=2&prefix=a%2Fb"
        );

        // Duplicate query names sort by encoded name then encoded value.
        assert_eq!(canonical_query("b=1&a=z&a=A&a=1"), "a=1&a=A&a=z&b=1");

        // Value containing `=`.
        assert_eq!(canonical_query("expr=a=b=c"), "expr=a%3Db%3Dc");

        // Invalid escape in query.
        assert_eq!(canonical_query("bad=%zz&trunc=%2"), "bad=%25zz&trunc=%252");

        // Empty segments from stray `&` are ignored.
        assert_eq!(canonical_query("&b=2&&a=1&"), "a=1&b=2");

        // Already-decoded pairs form does not percent-decode inputs.
        assert_eq!(
            canonical_query_pairs([
                (
                    "response-content-disposition",
                    "attachment; filename=\"a b.txt\"",
                ),
                ("versionId", "v1/2+3=4"),
                ("literalPercent", "%2F"),
            ]),
            "literalPercent=%252F&response-content-disposition=attachment%3B%20filename%3D%22a%20b.txt%22&versionId=v1%2F2%2B3%3D4"
        );

        // Idempotency for raw queries.
        for q in [
            "",
            "uploads",
            "partNumber=2&uploadId=abc%2Fdef",
            "list-type=2&prefix=a%2Fb&continuation-token=abc+def/ghi==",
            "expr=a=b=c",
            "a%26b=1%262",
            "a%3Db=1",
            "bad=%zz&trunc=%2",
            "b=1&a=z&a=A&a=1",
        ] {
            let once = canonical_query(q);
            assert_eq!(canonical_query(&once), once, "not idempotent for {q:?}");
        }
    }

    #[test]
    fn strip_presign_params_removes_all_variants_and_preserves_rest_verbatim() {
        assert_eq!(strip_presign_params(""), "");

        // Mixed case, encoded names, all 7 params removed; remaining untouched and in order.
        let q = concat!(
            "partNumber=2",
            "&X-Amz-Algorithm=AWS4-HMAC-SHA256",
            "&uploadId=abc%2fdef+1",
            "&x-amz-credential=AKID%2F20260101%2Fus-east-1%2Fs3%2Faws4_request",
            "&%58-Amz-Date=20260101T000000Z",
            "&X-AMZ-EXPIRES=3600",
            "&x%2Damz%2Dsignedheaders=host",
            "&x-amz-%53ignature=deadbeef",
            "&X-Amz-Security-Token=tok%2B1%3D",
            "&uploads",
            "&x-amz-meta-keep=raw+value%2fhere",
        );
        assert_eq!(
            strip_presign_params(q),
            "partNumber=2&uploadId=abc%2fdef+1&uploads&x-amz-meta-keep=raw+value%2fhere"
        );

        // Only presign params -> empty string.
        assert_eq!(
            strip_presign_params("X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=00"),
            ""
        );

        // Bare presign param without `=`.
        assert_eq!(strip_presign_params("x-amz-signature&keep=1"), "keep=1");

        // Duplicate presign params and lowercase hex escapes in names.
        assert_eq!(
            strip_presign_params(
                "x%2damz%2dalgorithm=A&keep=1&X-Amz-Signature=s1&x-amz-signature=s2"
            ),
            "keep=1"
        );

        // Nothing to strip -> returned verbatim.
        assert_eq!(
            strip_presign_params("z=1+2&a=foo%2fbar&bare"),
            "z=1+2&a=foo%2fbar&bare"
        );
    }

    #[test]
    fn duplicate_mixed_case_headers_merge_and_collapse_spaces() {
        let (canonical, signed) = canonicalize_headers(&[
            ("X-Custom", "  first   value  "),
            ("Host", "  example.com  "),
            ("x-custom", "second    value"),
        ])
        .unwrap();
        assert_eq!(signed, "host;x-custom");
        assert_eq!(
            canonical,
            "host:example.com\nx-custom:first value,second value\n"
        );
    }

    #[test]
    fn missing_host_and_invalid_headers_are_refused() {
        let creds = Credentials::new("AKID", "SUPER_SECRET_KEY", "us-east-1");
        let now = ts("2026-10-15T12:00:00Z");

        assert_eq!(
            authorization(
                "GET",
                "/k",
                "",
                &[("x-amz-date", "20261015T120000Z")],
                UNSIGNED_PAYLOAD,
                &creds,
                S3_SERVICE,
                now,
            ),
            Err(Error::MissingHost)
        );
        assert_eq!(
            authorization(
                "GET",
                "/k",
                "",
                &[("host", "   ")],
                UNSIGNED_PAYLOAD,
                &creds,
                S3_SERVICE,
                now,
            ),
            Err(Error::MissingHost)
        );
        assert_eq!(
            presign_query("GET", "", "/k", &[], 60, &creds, S3_SERVICE, now),
            Err(Error::MissingHost)
        );
        assert_eq!(
            authorization(
                "GET",
                "/k",
                "",
                &[("host", "example.com"), ("bad:name", "v")],
                UNSIGNED_PAYLOAD,
                &creds,
                S3_SERVICE,
                now,
            ),
            Err(Error::InvalidHeaderName)
        );
        assert_eq!(
            authorization(
                "GET",
                "/k",
                "",
                &[("host", "example.com"), ("x-bad", "line1\nline2")],
                UNSIGNED_PAYLOAD,
                &creds,
                S3_SERVICE,
                now,
            ),
            Err(Error::InvalidHeaderValue)
        );
    }

    #[test]
    fn presign_expiry_bounds_are_enforced() {
        let creds = Credentials::new("AKID", "SUPER_SECRET_KEY", "us-east-1");
        let now = ts("2026-10-15T12:00:00Z");

        assert_eq!(
            presign_query("GET", "example.com", "/k", &[], 0, &creds, S3_SERVICE, now),
            Err(Error::InvalidExpiry)
        );
        assert!(presign_query("GET", "example.com", "/k", &[], 1, &creds, S3_SERVICE, now).is_ok());
        assert!(
            presign_query(
                "GET",
                "example.com",
                "/k",
                &[],
                MAX_EXPIRES_SECS,
                &creds,
                S3_SERVICE,
                now,
            )
            .is_ok()
        );
        assert_eq!(
            presign_query(
                "GET",
                "example.com",
                "/k",
                &[],
                MAX_EXPIRES_SECS + 1,
                &creds,
                S3_SERVICE,
                now,
            ),
            Err(Error::InvalidExpiry)
        );
    }

    #[test]
    fn secret_never_appears_in_debug_or_error_output() {
        let secret = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
        let creds = Credentials::new("AKIAIOSFODNN7EXAMPLE", secret, "us-east-1");
        let debug = format!("{creds:?}");
        assert!(!debug.contains(secret), "{debug}");
        assert!(debug.contains("AKIAIOSFODNN7EXAMPLE"), "{debug}");

        for err in [
            Error::MissingHost,
            Error::InvalidExpiry,
            Error::InvalidHeaderName,
            Error::InvalidHeaderValue,
        ] {
            let dbg = format!("{err:?}");
            let disp = format!("{err}");
            assert!(!dbg.contains(secret));
            assert!(!disp.contains(secret));
            assert!(!disp.is_empty());
        }
    }

    #[test]
    fn unsigned_payload_header_signing_works() {
        let creds = Credentials::new("AKID", "SECRET", "eu-west-1");
        let now = ts("2026-10-15T12:00:00Z");
        let [sha, date, auth] = sign(
            "PUT",
            "bucket.s3.eu-west-1.amazonaws.com",
            "/dir/file+name.bin",
            "partNumber=3&uploadId=u%2Fid",
            UNSIGNED_PAYLOAD,
            &creds,
            S3_SERVICE,
            now,
        )
        .unwrap();
        assert_eq!(sha, ("x-amz-content-sha256", UNSIGNED_PAYLOAD.to_owned()));
        assert_eq!(date, ("x-amz-date", "20261015T120000Z".to_owned()));
        assert_eq!(auth.0, "authorization");
        assert!(auth.1.starts_with(
            "AWS4-HMAC-SHA256 Credential=AKID/20261015/eu-west-1/s3/aws4_request, \
             SignedHeaders=host;x-amz-content-sha256;x-amz-date, Signature="
        ));
    }
}
