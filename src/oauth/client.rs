//! Stateless OAuth 2.0 transport (reqwest). Behind the `oauth-client` feature.
//!
//! This is the shared *transport* layer both repos sit on: RFC 8414/9728/7591
//! URL building, the HTTP calls, form/JSON bodies, status checks, and parsing,
//! returning the shared DTOs. It is deliberately:
//!
//! - **SSRF-agnostic.** It does NOT resolve or gate hosts — the two repos guard
//!   differently (dyson async DNS-resolving predicate; swarm's IP-pinned/policy
//!   client). Callers validate endpoints in their own way *around* these calls.
//! - **Redaction-agnostic.** Errors are a structured [`OAuthError`] carrying raw
//!   fields; the reqwest-error case is reduced to a URL-free `kind` so nothing
//!   leaks by default. Each repo formats via [`OAuthError::redacted`] with its
//!   own redaction anchor (dyson: full URL + body; swarm: resource domain only,
//!   no body).
//! - **Stateless.** No token storage, refresh scheduling, callback server, or
//!   flow cache — those lifecycles stay in each repo on top of this.

use base64::Engine as _;
use rand::RngCore as _;
use sha2::{Digest as _, Sha256};

use super::{
    AuthMetadata, DcrRequest, DcrResponse, PkceChallenge, ProtectedResourceMetadata, TokenResponse,
};

const B64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE_NO_PAD;
const MAX_OAUTH_RESPONSE_BYTES: usize = 256 * 1024;

/// A transport-layer OAuth failure. Fields are raw; format via
/// [`OAuthError::redacted`] so the caller controls URL redaction + whether to
/// include the (potentially sensitive) response body.
#[derive(Debug, Clone)]
pub enum OAuthError {
    /// A URL we tried to build or parse was malformed.
    BadUrl { url: String, detail: String },
    /// Transport failure (connect/timeout/decode). `kind` is URL-free.
    Transport { url: String, kind: String },
    /// Non-2xx response. `body` may echo provider data — redact-aware callers
    /// decide whether to surface it.
    Status {
        url: String,
        code: u16,
        body: String,
    },
    /// 2xx but the body didn't deserialize.
    Parse { url: String, detail: String },
}

impl OAuthError {
    fn url(&self) -> &str {
        match self {
            Self::BadUrl { url, .. }
            | Self::Transport { url, .. }
            | Self::Status { url, .. }
            | Self::Parse { url, .. } => url,
        }
    }

    /// Render a log-safe message. `redact` maps the offending URL to the form
    /// the caller wants in logs (identity for full URL, or e.g. domain-only).
    /// `with_body` includes the response body on `Status` (off for callers
    /// whose URLs/bodies may carry tenant secrets).
    pub fn redacted(&self, redact: impl Fn(&str) -> String, with_body: bool) -> String {
        let where_ = redact(self.url());
        match self {
            Self::BadUrl { detail, .. } => format!("oauth: bad url {where_}: {detail}"),
            Self::Transport { kind, .. } => format!("oauth: {kind} to {where_}"),
            Self::Status { code, body, .. } => {
                if with_body {
                    format!("oauth: {where_} returned HTTP {code}: {body}")
                } else {
                    format!("oauth: {where_} returned HTTP {code}")
                }
            }
            Self::Parse { detail, .. } => format!("oauth: bad response from {where_}: {detail}"),
        }
    }
}

fn err_kind(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connect error"
    } else if e.is_decode() {
        "decode error"
    } else if e.is_request() {
        "request error"
    } else {
        "transport error"
    }
}

fn random_b64_32() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    B64.encode(bytes)
}

/// Generate a PKCE verifier + S256 challenge pair (RFC 7636).
pub fn generate_pkce() -> PkceChallenge {
    let verifier = random_b64_32();
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = B64.encode(hasher.finalize());
    PkceChallenge {
        verifier,
        challenge,
    }
}

/// Generate an opaque CSRF `state` value for the authorize redirect.
pub fn generate_state() -> String {
    random_b64_32()
}

/// RFC 8414 §3.1 authorization-server metadata URL. When the AS URL has a
/// non-trivial path, the well-known segment is inserted *between* origin and
/// path (multi-tenant providers); otherwise it sits at the root.
pub fn as_metadata_url(as_url: &str) -> Result<String, OAuthError> {
    let parsed = reqwest::Url::parse(as_url).map_err(|e| OAuthError::BadUrl {
        url: as_url.to_string(),
        detail: e.to_string(),
    })?;
    let host = parsed.host_str().ok_or_else(|| OAuthError::BadUrl {
        url: as_url.to_string(),
        detail: "no host".into(),
    })?;
    let origin = match parsed.port() {
        Some(p) => format!("{}://{host}:{p}", parsed.scheme()),
        None => format!("{}://{host}", parsed.scheme()),
    };
    let path = parsed.path().trim_end_matches('/');
    Ok(if path.is_empty() {
        format!("{origin}/.well-known/oauth-authorization-server")
    } else {
        format!("{origin}/.well-known/oauth-authorization-server{path}")
    })
}

/// RFC 9728 protected-resource metadata URLs to try, in order: the path-scoped
/// form (when the resource has a path) then the origin-root form.
pub fn protected_resource_metadata_urls(server_url: &str) -> Result<Vec<String>, OAuthError> {
    let parsed = reqwest::Url::parse(server_url).map_err(|e| OAuthError::BadUrl {
        url: server_url.to_string(),
        detail: e.to_string(),
    })?;
    let host = parsed.host_str().ok_or_else(|| OAuthError::BadUrl {
        url: server_url.to_string(),
        detail: "no host".into(),
    })?;
    let origin = match parsed.port() {
        Some(p) => format!("{}://{host}:{p}", parsed.scheme()),
        None => format!("{}://{host}", parsed.scheme()),
    };
    let path = parsed.path().trim_end_matches('/');
    let mut urls = Vec::new();
    if !path.is_empty() {
        urls.push(format!(
            "{origin}/.well-known/oauth-protected-resource{path}"
        ));
    }
    urls.push(format!("{origin}/.well-known/oauth-protected-resource"));
    Ok(urls)
}

/// Build the `/authorize` redirect URL with PKCE + state. Scopes are
/// space-joined; an empty slice omits the `scope` param entirely (some ASes
/// reject `scope=` / scopes not in the DCR record).
pub fn build_auth_url(
    authorization_endpoint: &str,
    client_id: &str,
    scopes: &[String],
    redirect_uri: &str,
    code_challenge: &str,
    state: &str,
) -> Result<String, OAuthError> {
    let mut url = reqwest::Url::parse(authorization_endpoint).map_err(|e| OAuthError::BadUrl {
        url: authorization_endpoint.to_string(),
        detail: e.to_string(),
    })?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("response_type", "code")
            .append_pair("client_id", client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("code_challenge", code_challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", state);
        if !scopes.is_empty() {
            q.append_pair("scope", &scopes.join(" "));
        }
    }
    Ok(url.to_string())
}

async fn get_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
) -> Result<T, OAuthError> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| OAuthError::Transport {
            url: url.to_string(),
            kind: err_kind(&e).into(),
        })?;
    let code = resp.status().as_u16();
    if !resp.status().is_success() {
        let body = response_text_capped(resp, url).await.unwrap_or_default();
        return Err(OAuthError::Status {
            url: url.to_string(),
            code,
            body,
        });
    }
    response_json_capped(resp, url).await
}

async fn response_bytes_capped(
    mut resp: reqwest::Response,
    url: &str,
) -> Result<Vec<u8>, OAuthError> {
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| OAuthError::Transport {
        url: url.to_string(),
        kind: err_kind(&e).into(),
    })? {
        if body.len().saturating_add(chunk.len()) > MAX_OAUTH_RESPONSE_BYTES {
            return Err(OAuthError::Parse {
                url: url.to_string(),
                detail: format!("response body exceeded {MAX_OAUTH_RESPONSE_BYTES} bytes"),
            });
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

async fn response_text_capped(resp: reqwest::Response, url: &str) -> Result<String, OAuthError> {
    let bytes = response_bytes_capped(resp, url).await?;
    String::from_utf8(bytes).map_err(|e| OAuthError::Parse {
        url: url.to_string(),
        detail: e.to_string(),
    })
}

async fn response_json_capped<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
    url: &str,
) -> Result<T, OAuthError> {
    let bytes = response_bytes_capped(resp, url).await?;
    serde_json::from_slice(&bytes).map_err(|e| OAuthError::Parse {
        url: url.to_string(),
        detail: e.to_string(),
    })
}

/// Fetch RFC 8414 AS metadata (builds the well-known URL via [`as_metadata_url`]).
pub async fn fetch_as_metadata(
    as_url: &str,
    client: &reqwest::Client,
) -> Result<AuthMetadata, OAuthError> {
    let url = as_metadata_url(as_url)?;
    get_json(client, &url).await
}

/// Fetch RFC 9728 protected-resource metadata and return its first listed
/// authorization server, if any. A 404 maps to `Ok(None)` (resource doesn't
/// publish PRM — caller falls back to treating the resource origin as the AS).
pub async fn fetch_protected_resource(
    prm_url: &str,
    client: &reqwest::Client,
) -> Result<Option<String>, OAuthError> {
    let resp = client
        .get(prm_url)
        .send()
        .await
        .map_err(|e| OAuthError::Transport {
            url: prm_url.to_string(),
            kind: err_kind(&e).into(),
        })?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let code = resp.status().as_u16();
    if !resp.status().is_success() {
        let body = response_text_capped(resp, prm_url)
            .await
            .unwrap_or_default();
        return Err(OAuthError::Status {
            url: prm_url.to_string(),
            code,
            body,
        });
    }
    let prm: ProtectedResourceMetadata = response_json_capped(resp, prm_url).await?;
    Ok(prm.authorization_servers.into_iter().next())
}

/// Register a client via RFC 7591 Dynamic Client Registration.
pub async fn register_client(
    registration_endpoint: &str,
    request: &DcrRequest,
    client: &reqwest::Client,
) -> Result<DcrResponse, OAuthError> {
    let resp = client
        .post(registration_endpoint)
        .json(request)
        .send()
        .await
        .map_err(|e| OAuthError::Transport {
            url: registration_endpoint.to_string(),
            kind: err_kind(&e).into(),
        })?;
    let code = resp.status().as_u16();
    if !resp.status().is_success() {
        let body = response_text_capped(resp, registration_endpoint)
            .await
            .unwrap_or_default();
        return Err(OAuthError::Status {
            url: registration_endpoint.to_string(),
            code,
            body,
        });
    }
    response_json_capped(resp, registration_endpoint).await
}

async fn post_token(
    token_url: &str,
    params: &[(&str, &str)],
    client: &reqwest::Client,
) -> Result<TokenResponse, OAuthError> {
    let resp = client
        .post(token_url)
        .form(params)
        .send()
        .await
        .map_err(|e| OAuthError::Transport {
            url: token_url.to_string(),
            kind: err_kind(&e).into(),
        })?;
    let code = resp.status().as_u16();
    if !resp.status().is_success() {
        let body = response_text_capped(resp, token_url)
            .await
            .unwrap_or_default();
        return Err(OAuthError::Status {
            url: token_url.to_string(),
            code,
            body,
        });
    }
    response_json_capped(resp, token_url).await
}

/// Exchange an authorization code for tokens (RFC 6749 §4.1.3).
#[allow(clippy::too_many_arguments)]
pub async fn exchange_code(
    token_url: &str,
    code: &str,
    verifier: &str,
    client_id: &str,
    client_secret: Option<&str>,
    redirect_uri: &str,
    client: &reqwest::Client,
) -> Result<TokenResponse, OAuthError> {
    let mut params = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("code_verifier", verifier),
    ];
    if let Some(secret) = client_secret {
        params.push(("client_secret", secret));
    }
    post_token(token_url, &params, client).await
}

/// Refresh an access token (RFC 6749 §6).
pub async fn refresh_token(
    token_url: &str,
    refresh_token: &str,
    client_id: &str,
    client_secret: Option<&str>,
    client: &reqwest::Client,
) -> Result<TokenResponse, OAuthError> {
    let mut params = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
    ];
    if let Some(secret) = client_secret {
        params.push(("client_secret", secret));
    }
    post_token(token_url, &params, client).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_is_random_and_s256_derived() {
        let a = generate_pkce();
        assert_ne!(a.verifier, generate_pkce().verifier);
        let mut h = Sha256::new();
        h.update(a.verifier.as_bytes());
        assert_eq!(a.challenge, B64.encode(h.finalize()));
    }

    #[test]
    fn as_metadata_url_is_rfc8414_path_aware() {
        assert_eq!(
            as_metadata_url("https://as.example.com").unwrap(),
            "https://as.example.com/.well-known/oauth-authorization-server"
        );
        assert_eq!(
            as_metadata_url("https://as.example.com/").unwrap(),
            "https://as.example.com/.well-known/oauth-authorization-server"
        );
        // path-bearing issuer: well-known inserted between origin and path
        assert_eq!(
            as_metadata_url("https://as.example.com/tenant/svc").unwrap(),
            "https://as.example.com/.well-known/oauth-authorization-server/tenant/svc"
        );
        assert!(matches!(
            as_metadata_url("not a url"),
            Err(OAuthError::BadUrl { .. })
        ));
    }

    #[test]
    fn protected_resource_urls_try_path_then_root() {
        assert_eq!(
            protected_resource_metadata_urls("https://mcp.example.com/x/y").unwrap(),
            vec![
                "https://mcp.example.com/.well-known/oauth-protected-resource/x/y".to_string(),
                "https://mcp.example.com/.well-known/oauth-protected-resource".to_string(),
            ]
        );
        assert_eq!(
            protected_resource_metadata_urls("https://mcp.example.com").unwrap(),
            vec!["https://mcp.example.com/.well-known/oauth-protected-resource".to_string()]
        );
    }

    #[test]
    fn build_auth_url_omits_empty_scope() {
        let pkce = PkceChallenge {
            verifier: "v".into(),
            challenge: "chal".into(),
        };
        let with = build_auth_url(
            "https://as/authorize",
            "cid",
            &["openid".to_string(), "mcp".to_string()],
            "http://localhost/cb",
            &pkce.challenge,
            "st",
        )
        .unwrap();
        assert!(with.contains("scope=openid+mcp") || with.contains("scope=openid%20mcp"));
        assert!(with.contains("code_challenge=chal") && with.contains("state=st"));

        let without = build_auth_url(
            "https://as/authorize",
            "cid",
            &[],
            "http://localhost/cb",
            "chal",
            "st",
        )
        .unwrap();
        assert!(!without.contains("scope="));
    }

    #[test]
    fn redacted_honours_anchor_and_body_flag() {
        let e = OAuthError::Status {
            url: "https://tenant.smithery.ai/secret-path?key=abc".into(),
            code: 403,
            body: "forbidden: token xyz".into(),
        };
        // dyson-style: full URL + body
        let full = e.redacted(|u| u.to_string(), true);
        assert!(full.contains("secret-path") && full.contains("forbidden: token xyz"));
        // swarm-style: fixed resource domain, no body
        let red = e.redacted(|_| "mcp.example.com".to_string(), false);
        assert!(red.contains("mcp.example.com") && red.contains("403"));
        assert!(!red.contains("secret-path") && !red.contains("xyz"));
    }
}
