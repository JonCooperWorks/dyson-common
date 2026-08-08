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
    #[error("Telegram file exceeds {limit} byte limit")]
    TooLarge { limit: u64 },
    #[error("Telegram file size overflow")]
    SizeOverflow,
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

    let mut request = client.get(format!(
        "{}/{}",
        file_base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    ));
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
        .unwrap_or_default();
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
}
