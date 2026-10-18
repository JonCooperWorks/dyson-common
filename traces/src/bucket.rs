//! A bucket behind an S3 gateway, addressed by path: where session files
//! are put. Hugging Face's Storage Buckets are reached at
//! `https://s3.hf.co`, a bucket there named `<owner>/<name>`.

use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;

use crate::sigv4::{self, Credentials};

/// Hugging Face's S3 gateway.
pub const HF_ENDPOINT: &str = "https://s3.hf.co";
/// The region HF's gateway signs for.
pub const HF_REGION: &str = "us-east-1";
/// The most of one file [`Sink::get`] reads.
pub const GET_MOST: usize = 32 * 1024 * 1024;

/// Why a file could not be put, or the bucket not reached.
#[derive(Debug)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// Where session files go. A test can keep them in memory.
#[async_trait]
pub trait Sink: Send + Sync {
    async fn put(&self, key: &str, body: Vec<u8>) -> Result<(), Error>;
    /// Gone already is not an error.
    async fn delete(&self, key: &str) -> Result<(), Error>;
    /// A file read back, `None` when there is none. A sink that cannot
    /// read says so.
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        let _ = key;
        Err(Error(format!("{} cannot read files back", self.describe())))
    }
    /// Where this is, for an operator to read.
    fn describe(&self) -> String;
}

/// Where a bucket is and its keys.
#[derive(Debug, Clone)]
pub struct Destination {
    /// `https` only.
    pub endpoint: String,
    pub bucket: String,
    pub credentials: Credentials,
}

impl Destination {
    /// A Hugging Face Storage Bucket, `<owner>/<name>`.
    pub fn hugging_face(
        bucket: impl Into<String>,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: HF_ENDPOINT.into(),
            bucket: bucket.into(),
            credentials: Credentials {
                access_key_id: access_key_id.into(),
                secret_access_key: secret_access_key.into(),
                region: HF_REGION.into(),
            },
        }
    }
}

pub struct Bucket {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    bucket: String,
    credentials: Credentials,
}

impl Bucket {
    pub fn new(destination: &Destination) -> Result<Self, Error> {
        let endpoint: reqwest::Url = destination
            .endpoint
            .parse()
            .map_err(|_| Error("the bucket's endpoint is not a URL".into()))?;
        if endpoint.scheme() != "https" {
            return Err(Error("the bucket's endpoint must be https".into()));
        }
        let plain = |s: &str| {
            !s.is_empty()
                && s.split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'))
        };
        if !plain(&destination.bucket) {
            return Err(Error("a bucket name of plain characters".into()));
        }
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .map_err(|e| Error(format!("build the bucket's client: {e}")))?,
            endpoint,
            bucket: destination.bucket.clone(),
            credentials: destination.credentials.clone(),
        })
    }

    fn url(&self, key: &str) -> Result<reqwest::Url, Error> {
        let plain = !key.is_empty()
            && key
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != "..")
            && key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'));
        if !plain {
            return Err(Error("an object key of plain characters".into()));
        }
        let endpoint = self.endpoint.as_str().trim_end_matches('/');
        format!("{endpoint}/{}/{key}", self.bucket)
            .parse()
            .map_err(|_| Error(format!("no URL for {key}")))
    }

    /// One signed request. The body of a successful GET comes back; a
    /// GET or DELETE of nothing is `None`.
    async fn send(
        &self,
        method: reqwest::Method,
        key: &str,
        body: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, Error> {
        let url = self.url(key)?;
        let host = match url.port() {
            Some(port) => format!("{}:{port}", url.host_str().unwrap_or_default()),
            None => url.host_str().unwrap_or_default().to_owned(),
        };
        let signed = sigv4::sign(
            method.as_str(),
            &host,
            url.path(),
            &body,
            &self.credentials,
            Utc::now(),
        );
        let mut request = self.client.request(method.clone(), url);
        for (name, value) in signed {
            request = request.header(name, value);
        }
        if method == reqwest::Method::PUT {
            request = request
                .header("content-type", "application/x-ndjson")
                .body(body);
        }
        let mut response = request
            .send()
            .await
            .map_err(|e| Error(format!("reach the bucket: {e}")))?;
        let status = response.status();
        if status.as_u16() == 404 && method != reqwest::Method::PUT {
            return Ok(None);
        }
        if status.is_success() {
            if method != reqwest::Method::GET {
                return Ok(Some(Vec::new()));
            }
            if response
                .content_length()
                .is_some_and(|n| n > GET_MOST as u64)
            {
                return Err(Error(format!("{key} is larger than {GET_MOST} bytes")));
            }
            let mut out = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|e| Error(format!("read {key}: {e}")))?
            {
                if out.len() + chunk.len() > GET_MOST {
                    return Err(Error(format!("{key} is larger than {GET_MOST} bytes")));
                }
                out.extend_from_slice(&chunk);
            }
            return Ok(Some(out));
        }
        let said: String = response
            .text()
            .await
            .unwrap_or_default()
            .chars()
            .take(300)
            .collect();
        Err(Error(format!(
            "the bucket answered {status} to {method} {key}: {said}"
        )))
    }
}

#[async_trait]
impl Sink for Bucket {
    async fn put(&self, key: &str, body: Vec<u8>) -> Result<(), Error> {
        self.send(reqwest::Method::PUT, key, body).await.map(drop)
    }

    async fn delete(&self, key: &str) -> Result<(), Error> {
        self.send(reqwest::Method::DELETE, key, Vec::new())
            .await
            .map(drop)
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Error> {
        self.send(reqwest::Method::GET, key, Vec::new()).await
    }

    fn describe(&self) -> String {
        format!(
            "{}/{}",
            self.endpoint.as_str().trim_end_matches('/'),
            self.bucket
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bucket(endpoint: &str, name: &str) -> Result<Bucket, Error> {
        let mut destination = Destination::hugging_face(name, "id", "secret");
        destination.endpoint = endpoint.into();
        Bucket::new(&destination)
    }

    #[test]
    fn addresses_a_bucket_by_path() {
        let bucket = bucket("https://s3.hf.co/", "owner/traces").unwrap();
        assert_eq!(
            bucket.url("sessions/2026/10/15/x.jsonl").unwrap().as_str(),
            "https://s3.hf.co/owner/traces/sessions/2026/10/15/x.jsonl"
        );
        assert_eq!(bucket.describe(), "https://s3.hf.co/owner/traces");
    }

    #[test]
    fn refuses_what_would_need_encoding_or_climb_out() {
        let bucket = bucket(HF_ENDPOINT, "owner/traces").unwrap();
        for key in ["", "a b", "../x", "a//b", "a/./b", "a?b=c", "é"] {
            assert!(bucket.url(key).is_err(), "{key}");
        }
        assert!(super::tests::bucket("http://s3.hf.co", "o/b").is_err());
        assert!(super::tests::bucket(HF_ENDPOINT, "o/../b").is_err());
        assert!(super::tests::bucket("not a url", "o/b").is_err());
    }
}
