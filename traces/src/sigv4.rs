//! AWS Signature Version 4 for S3 requests, as plain functions: no HTTP
//! client, so a host can sign with whatever client it already has.
//!
//! Thin compatibility layer over [`dyson_sigv4`], fixed to the `s3` service
//! with no query string and a whole-payload SHA-256.

use chrono::{DateTime, Utc};

pub use dyson_sigv4::{Credentials, amz_date, payload_sha256};

/// An `Authorization` header for S3. `headers` are the ones signed (including
/// `host`); `path` is as it is sent.
pub fn authorization(
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    payload_sha256: &str,
    credentials: &Credentials,
    now: DateTime<Utc>,
) -> String {
    dyson_sigv4::authorization(
        method,
        path,
        "",
        headers,
        payload_sha256,
        credentials,
        dyson_sigv4::S3_SERVICE,
        now,
    )
    .expect("valid SigV4 signing headers including host")
}

/// The headers that sign a request with no others: `host` is what the
/// client sends (with the port when it is not the scheme's), and is not
/// returned. Set `x-amz-content-sha256`, `x-amz-date` and `authorization`.
pub fn sign(
    method: &str,
    host: &str,
    path: &str,
    body: &[u8],
    credentials: &Credentials,
    now: DateTime<Utc>,
) -> [(&'static str, String); 3] {
    let payload = payload_sha256(body);
    dyson_sigv4::sign(
        method,
        host,
        path,
        "",
        &payload,
        credentials,
        dyson_sigv4::S3_SERVICE,
        now,
    )
    .expect("valid SigV4 host header")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example in "Authenticating Requests: Using the Authorization
    /// Header (AWS Signature Version 4)", GET Object.
    #[test]
    fn signs_as_the_s3_documentation_does() {
        let now = DateTime::parse_from_rfc3339("2013-05-24T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let empty = payload_sha256(b"");
        assert_eq!(
            empty,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let credentials = Credentials {
            access_key_id: "AKIAIOSFODNN7EXAMPLE".into(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into(),
            region: "us-east-1".into(),
        };
        let header = authorization(
            "GET",
            "/test.txt",
            &[
                ("host", "examplebucket.s3.amazonaws.com"),
                ("range", "bytes=0-9"),
                ("x-amz-content-sha256", &empty),
                ("x-amz-date", "20130524T000000Z"),
            ],
            &empty,
            &credentials,
            now,
        );
        assert_eq!(
            header,
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
             SignedHeaders=host;range;x-amz-content-sha256;x-amz-date, \
             Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
        );
        assert!(!format!("{credentials:?}").contains("EXAMPLEKEY"));
    }

    #[test]
    fn sign_returns_the_headers_it_signed() {
        let now = DateTime::parse_from_rfc3339("2026-10-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let credentials = Credentials {
            access_key_id: "id".into(),
            secret_access_key: "secret".into(),
            region: "us-east-1".into(),
        };
        let [sha, date, auth] = sign("PUT", "s3.hf.co", "/o/b/k.jsonl", b"x", &credentials, now);
        assert_eq!(sha, ("x-amz-content-sha256", payload_sha256(b"x")));
        assert_eq!(date, ("x-amz-date", "20261015T120000Z".to_owned()));
        assert_eq!(auth.0, "authorization");
        assert!(
            auth.1
                .contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date")
        );
        assert!(
            auth.1
                .contains("Credential=id/20261015/us-east-1/s3/aws4_request")
        );
    }
}
