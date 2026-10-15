//! AWS Signature Version 4 for S3 requests, as plain functions: no HTTP
//! client, so a host can sign with whatever client it already has.
//!
//! Only what a bucket of session files needs: path-style requests whose
//! object keys hold no character that needs percent-encoding (see
//! [`crate::object_key`]), no query string, the payload hashed whole.

use chrono::{DateTime, Utc};
use sha2::{Digest as _, Sha256};

/// A bucket's keys. `Debug` never shows the secret.
#[derive(Clone)]
pub struct Credentials {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"…")
            .field("region", &self.region)
            .finish()
    }
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::Mac as _;
    let mut mac = hmac::Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// The hex SHA-256 of a payload, as `x-amz-content-sha256` carries it.
pub fn payload_sha256(body: &[u8]) -> String {
    hex::encode(Sha256::digest(body))
}

/// The `x-amz-date` of a moment.
pub fn amz_date(now: DateTime<Utc>) -> String {
    now.format("%Y%m%dT%H%M%SZ").to_string()
}

/// An `Authorization` header for S3. `headers` are the ones signed,
/// lowercase and in order of name; `path` is as it is sent.
pub fn authorization(
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    payload_sha256: &str,
    credentials: &Credentials,
    now: DateTime<Utc>,
) -> String {
    let date = now.format("%Y%m%d").to_string();
    let stamp = amz_date(now);
    let region = credentials.region.as_str();
    let signed: Vec<&str> = headers.iter().map(|(name, _)| *name).collect();
    let signed = signed.join(";");
    let canonical_headers: String = headers
        .iter()
        .map(|(name, value)| format!("{name}:{}\n", value.trim()))
        .collect();
    let canonical = format!("{method}\n{path}\n\n{canonical_headers}\n{signed}\n{payload_sha256}");
    let scope = format!("{date}/{region}/s3/aws4_request");
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{stamp}\n{scope}\n{}",
        hex::encode(Sha256::digest(canonical.as_bytes()))
    );
    let mut key = hmac(
        format!("AWS4{}", credentials.secret_access_key).as_bytes(),
        date.as_bytes(),
    );
    for part in [region, "s3", "aws4_request"] {
        key = hmac(&key, part.as_bytes());
    }
    let signature = hex::encode(hmac(&key, to_sign.as_bytes()));
    format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed}, Signature={signature}",
        credentials.access_key_id
    )
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
    let stamp = amz_date(now);
    let authorization = authorization(
        method,
        path,
        &[
            ("host", host),
            ("x-amz-content-sha256", &payload),
            ("x-amz-date", &stamp),
        ],
        &payload,
        credentials,
        now,
    );
    [
        ("x-amz-content-sha256", payload),
        ("x-amz-date", stamp),
        ("authorization", authorization),
    ]
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
