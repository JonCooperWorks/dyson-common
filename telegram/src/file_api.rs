use futures_util::StreamExt as _;
use serde::Deserialize;

#[derive(Debug, thiserror::Error)]
pub enum FileDownloadError {
    #[error("Telegram {stage} request failed")]
    Transport { stage: &'static str },
    #[error("Telegram {stage} returned HTTP {status}")]
    Http {
        stage: &'static str,
        status: reqwest::StatusCode,
    },
    #[error("Telegram getFile failed: {0}")]
    Telegram(String),
    #[error("Telegram getFile response missing file_path")]
    MissingPath,
    #[error("Telegram getFile response has invalid file_path")]
    InvalidPath,
    #[error("Telegram file exceeds {limit} byte limit")]
    TooLarge { limit: u64 },
    #[error("Telegram file size overflow")]
    SizeOverflow,
}

/// Cap on initial `Vec::with_capacity` derived from an untrusted
/// `Content-Length` header so a lying response cannot force a large
/// upfront allocation before streaming any bytes.
const MAX_INITIAL_CAPACITY: usize = 64 * 1024;

fn build_file_url(file_base_url: &str, raw_path: &str) -> Result<reqwest::Url, FileDownloadError> {
    let trimmed = raw_path.strip_prefix('/').unwrap_or(raw_path);
    if trimmed.is_empty() {
        return Err(FileDownloadError::MissingPath);
    }
    let mut segments = Vec::new();
    for seg in trimmed.split('/') {
        if seg.is_empty()
            || seg == "."
            || seg == ".."
            || seg.contains(['\\', '?', '#', '@'])
            || seg.bytes().any(|b| b < 0x20 || b == 0x7f)
        {
            return Err(FileDownloadError::InvalidPath);
        }
        segments.push(seg);
    }
    let mut url = reqwest::Url::parse(file_base_url.trim_end_matches('/')).map_err(|_| {
        FileDownloadError::Transport {
            stage: "file download",
        }
    })?;
    {
        let mut path_segments =
            url.path_segments_mut()
                .map_err(|()| FileDownloadError::Transport {
                    stage: "file download",
                })?;
        path_segments.pop_if_empty();
        for seg in segments {
            path_segments.push(seg);
        }
    }
    Ok(url)
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    #[serde(default)]
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct File {
    #[serde(default)]
    file_size: Option<u64>,
    #[serde(default)]
    file_path: Option<String>,
}

/// Resolve and stream a Telegram file with a hard memory bound.
///
/// `method_base_url` is the Telegram-shaped bot endpoint without a trailing
/// slash; `file_base_url` is its file endpoint. A bearer is optional because
/// direct Bot API URLs carry the bot token while Swarm's proxy authenticates
/// with an instance bearer. Transport errors deliberately omit URLs so a
/// direct-mode bot token can never leak through an error string.
pub async fn download_file(
    client: &reqwest::Client,
    method_base_url: &str,
    file_base_url: &str,
    bearer: Option<&str>,
    file_id: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, FileDownloadError> {
    let mut request = client
        .post(format!("{}/getFile", method_base_url.trim_end_matches('/')))
        .json(&serde_json::json!({ "file_id": file_id }));
    if let Some(token) = bearer {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(|_| FileDownloadError::Transport { stage: "getFile" })?;
    if !response.status().is_success() {
        return Err(FileDownloadError::Http {
            stage: "getFile",
            status: response.status(),
        });
    }
    let envelope: Envelope<File> = response
        .json()
        .await
        .map_err(|_| FileDownloadError::Transport { stage: "getFile" })?;
    if !envelope.ok {
        return Err(FileDownloadError::Telegram(
            envelope
                .description
                .unwrap_or_else(|| "unknown error".into()),
        ));
    }
    let file = envelope
        .result
        .ok_or_else(|| FileDownloadError::Telegram("response missing result".into()))?;
    if file.file_size.is_some_and(|size| size > max_bytes) {
        return Err(FileDownloadError::TooLarge { limit: max_bytes });
    }
    let path = file
        .file_path
        .filter(|path| !path.is_empty())
        .ok_or(FileDownloadError::MissingPath)?;
    let file_url = build_file_url(file_base_url, &path)?;

    let mut request = client.get(file_url);
    if let Some(token) = bearer {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(|_| FileDownloadError::Transport {
            stage: "file download",
        })?;
    if !response.status().is_success() {
        return Err(FileDownloadError::Http {
            stage: "file download",
            status: response.status(),
        });
    }
    if response
        .content_length()
        .is_some_and(|size| size > max_bytes)
    {
        return Err(FileDownloadError::TooLarge { limit: max_bytes });
    }
    let capacity = response
        .content_length()
        .and_then(|size| usize::try_from(size).ok())
        .unwrap_or_default()
        .min(MAX_INITIAL_CAPACITY);
    let mut bytes = Vec::with_capacity(capacity);
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| FileDownloadError::Transport {
            stage: "file download",
        })?;
        let next_len = bytes
            .len()
            .checked_add(chunk.len())
            .ok_or(FileDownloadError::SizeOverflow)?;
        if u64::try_from(next_len).unwrap_or(u64::MAX) > max_bytes {
            return Err(FileDownloadError::TooLarge { limit: max_bytes });
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        routing::{get, post},
    };

    async fn server(file_size: Option<u64>, body: &'static [u8]) -> String {
        let app = Router::new()
            .route(
                "/bot/getFile",
                post(move || async move {
                    Json(serde_json::json!({
                        "ok": true,
                        "result": { "file_size": file_size, "file_path": "docs/a.txt" }
                    }))
                }),
            )
            .route("/file/docs/a.txt", get(move || async move { body }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn downloads_within_limit() {
        let base = server(Some(5), b"hello").await;
        let client = reqwest::Client::new();
        assert_eq!(
            download_file(
                &client,
                &format!("{base}/bot"),
                &format!("{base}/file"),
                None,
                "id",
                5,
            )
            .await
            .unwrap(),
            b"hello"
        );
    }

    #[tokio::test]
    async fn rejects_reported_oversize_before_file_fetch() {
        let base = server(Some(6), b"hello!").await;
        let client = reqwest::Client::new();
        assert!(matches!(
            download_file(
                &client,
                &format!("{base}/bot"),
                &format!("{base}/file"),
                None,
                "id",
                5,
            )
            .await,
            Err(FileDownloadError::TooLarge { .. })
        ));
    }

    #[test]
    fn build_file_url_rejects_traversal_and_special_delimiters() {
        let base = "https://api.telegram.org/file/bot123";
        assert_eq!(
            build_file_url(base, "docs/a.txt").unwrap().as_str(),
            "https://api.telegram.org/file/bot123/docs/a.txt"
        );
        assert_eq!(
            build_file_url(base, "/photos/file_0.jpg").unwrap().as_str(),
            "https://api.telegram.org/file/bot123/photos/file_0.jpg"
        );
        for bad in [
            "../secret",
            "docs/../secret",
            "docs/./a.txt",
            "//evil.example/a",
            "docs//a.txt",
            "docs\\a.txt",
            "docs/a.txt?extra=1",
            "docs/a.txt#frag",
            "@evil.example/a",
            "docs/a\r\nb.txt",
        ] {
            assert!(
                matches!(
                    build_file_url(base, bad),
                    Err(FileDownloadError::InvalidPath)
                ),
                "expected InvalidPath for {bad:?}"
            );
        }
        assert!(matches!(
            build_file_url(base, "/"),
            Err(FileDownloadError::MissingPath)
        ));
    }

    #[test]
    fn build_file_url_keeps_base_path_and_encodes_legitimate_names() {
        // The real endpoint: the token's colon stays in its segment.
        let telegram = "https://api.telegram.org/file/bot123:ABC-def/";
        assert_eq!(
            build_file_url(telegram, "documents/file with spaces.pdf")
                .unwrap()
                .as_str(),
            "https://api.telegram.org/file/bot123:ABC-def/documents/file%20with%20spaces.pdf"
        );
        assert_eq!(
            build_file_url(telegram, "documents/résumé 東京.pdf")
                .unwrap()
                .as_str(),
            "https://api.telegram.org/file/bot123:ABC-def/documents/r%C3%A9sum%C3%A9%20%E6%9D%B1%E4%BA%AC.pdf"
        );
        // A literal percent is part of the name, not an escape.
        assert_eq!(
            build_file_url(telegram, "documents/100%25.txt")
                .unwrap()
                .as_str(),
            "https://api.telegram.org/file/bot123:ABC-def/documents/100%2525.txt"
        );
        // The Swarm proxy file base, as Dyson is configured with it.
        assert_eq!(
            build_file_url(
                "https://swarm.test/v1/proxy/telegram/i-1/file",
                "photos/file_0.jpg"
            )
            .unwrap()
            .as_str(),
            "https://swarm.test/v1/proxy/telegram/i-1/file/photos/file_0.jpg"
        );
        // A local Bot API server reports an absolute path.
        assert_eq!(
            build_file_url(
                "http://127.0.0.1:8081/file/bot1",
                "/var/lib/tg/bot1/voice/file_3.oga"
            )
            .unwrap()
            .as_str(),
            "http://127.0.0.1:8081/file/bot1/var/lib/tg/bot1/voice/file_3.oga"
        );
        assert!(matches!(
            build_file_url("", "photos/file_0.jpg"),
            Err(FileDownloadError::Transport { .. })
        ));
    }

    #[tokio::test]
    async fn downloads_a_path_with_spaces() {
        let app = Router::new()
            .route(
                "/bot/getFile",
                post(|| async {
                    Json(serde_json::json!({
                        "ok": true,
                        "result": { "file_size": 64, "file_path": "documents/my file.txt" }
                    }))
                }),
            )
            .route(
                "/file/{*path}",
                get(|axum::extract::Path(path): axum::extract::Path<String>| async move { path }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let base = format!("http://{address}");
        let client = reqwest::Client::new();
        assert_eq!(
            download_file(
                &client,
                &format!("{base}/bot"),
                &format!("{base}/file/"),
                None,
                "id",
                64,
            )
            .await
            .unwrap(),
            b"documents/my file.txt"
        );
    }
}
