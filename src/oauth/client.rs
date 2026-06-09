//! OAuth 2.0 flow client (reqwest). Behind the `oauth-client` feature.
//!
//! Pure, dependency-light implementation of the MCP-auth flow shared by both
//! repos: metadata discovery (RFC 8414 + RFC 9728), Dynamic Client Registration
//! (RFC 7591), PKCE (RFC 7636), and the authorization-code + refresh grants
//! (RFC 6749).
//!
//! SSRF is left to the caller: every networked call takes an `allow_url`
//! predicate (return `true` to permit). Each repo plugs in its own
//! internal-host policy — dyson's fixed SSRF predicates, swarm's egress CIDR —
//! so this crate stays free of either model. Pass `|_| true` only when the
//! endpoint is already trusted.

use base64::Engine as _;
use rand::RngCore as _;
use sha2::{Digest as _, Sha256};

use super::{
    AuthMetadata, DcrRequest, DcrResponse, PkceChallenge, ProtectedResourceMetadata, TokenResponse,
};

const B64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// Failure modes of the OAuth flow.
#[derive(Debug)]
pub enum OAuthError {
    /// Transport-level failure (connect, TLS, body read).
    Http(reqwest::Error),
    /// The server returned a non-2xx status.
    Status(u16),
    /// A URL or response payload was malformed.
    Malformed(String),
    /// The `allow_url` predicate rejected the target as internal/unsafe.
    BlockedUrl(String),
}

impl std::fmt::Display for OAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "oauth http error: {e}"),
            Self::Status(c) => write!(f, "oauth endpoint returned status {c}"),
            Self::Malformed(m) => write!(f, "oauth malformed: {m}"),
            Self::BlockedUrl(u) => write!(f, "oauth url blocked as internal: {u}"),
        }
    }
}

impl std::error::Error for OAuthError {}

impl From<reqwest::Error> for OAuthError {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e)
    }
}

fn random_b64_32() -> String {
    let mut bytes = [0u8; 32];
    let mut rng = rand::rngs::OsRng;
    rng.fill_bytes(&mut bytes);
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

/// Build the `/authorize` redirect URL with PKCE + state (RFC 6749 §4.1.1).
pub fn build_auth_url(
    authorization_endpoint: &str,
    client_id: &str,
    redirect_uri: &str,
    scope: Option<&str>,
    pkce: &PkceChallenge,
    state: &str,
) -> Result<String, OAuthError> {
    let mut params: Vec<(&str, &str)> = vec![
        ("response_type", "code"),
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("code_challenge", pkce.challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("state", state),
    ];
    if let Some(s) = scope {
        params.push(("scope", s));
    }
    let url = reqwest::Url::parse_with_params(authorization_endpoint, &params)
        .map_err(|e| OAuthError::Malformed(format!("authorize url: {e}")))?;
    Ok(url.to_string())
}

fn ensure_allowed(allow_url: &impl Fn(&str) -> bool, url: &str) -> Result<(), OAuthError> {
    if allow_url(url) {
        Ok(())
    } else {
        Err(OAuthError::BlockedUrl(url.to_string()))
    }
}

fn err_for_status(status: reqwest::StatusCode) -> Result<(), OAuthError> {
    if status.is_success() {
        Ok(())
    } else {
        Err(OAuthError::Status(status.as_u16()))
    }
}

/// Fetch RFC 8414 authorization-server metadata from
/// `<issuer>/.well-known/oauth-authorization-server`.
pub async fn discover_metadata(
    issuer: &str,
    client: &reqwest::Client,
    allow_url: impl Fn(&str) -> bool,
) -> Result<AuthMetadata, OAuthError> {
    let url = format!(
        "{}/.well-known/oauth-authorization-server",
        issuer.trim_end_matches('/')
    );
    ensure_allowed(&allow_url, &url)?;
    let resp = client.get(&url).send().await?;
    err_for_status(resp.status())?;
    Ok(resp.json::<AuthMetadata>().await?)
}

/// Fetch RFC 9728 protected-resource metadata from
/// `<resource>/.well-known/oauth-protected-resource`.
pub async fn fetch_protected_resource(
    resource_url: &str,
    client: &reqwest::Client,
    allow_url: impl Fn(&str) -> bool,
) -> Result<ProtectedResourceMetadata, OAuthError> {
    let url = format!(
        "{}/.well-known/oauth-protected-resource",
        resource_url.trim_end_matches('/')
    );
    ensure_allowed(&allow_url, &url)?;
    let resp = client.get(&url).send().await?;
    err_for_status(resp.status())?;
    Ok(resp.json::<ProtectedResourceMetadata>().await?)
}

/// Register a client via RFC 7591 Dynamic Client Registration.
pub async fn register_client(
    registration_endpoint: &str,
    request: &DcrRequest,
    client: &reqwest::Client,
    allow_url: impl Fn(&str) -> bool,
) -> Result<DcrResponse, OAuthError> {
    ensure_allowed(&allow_url, registration_endpoint)?;
    let resp = client
        .post(registration_endpoint)
        .json(request)
        .send()
        .await?;
    err_for_status(resp.status())?;
    Ok(resp.json::<DcrResponse>().await?)
}

/// Exchange an authorization code for tokens (RFC 6749 §4.1.3).
#[allow(clippy::too_many_arguments)]
pub async fn exchange_code(
    token_endpoint: &str,
    code: &str,
    redirect_uri: &str,
    client_id: &str,
    code_verifier: &str,
    client_secret: Option<&str>,
    client: &reqwest::Client,
    allow_url: impl Fn(&str) -> bool,
) -> Result<TokenResponse, OAuthError> {
    ensure_allowed(&allow_url, token_endpoint)?;
    let mut form: Vec<(&str, &str)> = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", client_id),
        ("code_verifier", code_verifier),
    ];
    if let Some(secret) = client_secret {
        form.push(("client_secret", secret));
    }
    let resp = client.post(token_endpoint).form(&form).send().await?;
    err_for_status(resp.status())?;
    Ok(resp.json::<TokenResponse>().await?)
}

/// Refresh an access token (RFC 6749 §6).
pub async fn refresh_token(
    token_endpoint: &str,
    refresh_token: &str,
    client_id: &str,
    client_secret: Option<&str>,
    client: &reqwest::Client,
    allow_url: impl Fn(&str) -> bool,
) -> Result<TokenResponse, OAuthError> {
    ensure_allowed(&allow_url, token_endpoint)?;
    let mut form: Vec<(&str, &str)> = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
    ];
    if let Some(secret) = client_secret {
        form.push(("client_secret", secret));
    }
    let resp = client.post(token_endpoint).form(&form).send().await?;
    err_for_status(resp.status())?;
    Ok(resp.json::<TokenResponse>().await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_is_random_and_s256_derived() {
        let a = generate_pkce();
        let b = generate_pkce();
        assert!(!a.verifier.is_empty() && !a.challenge.is_empty());
        assert_ne!(a.verifier, a.challenge);
        assert_ne!(a.verifier, b.verifier, "verifiers must be random per call");

        // challenge == base64url(sha256(verifier))
        let mut h = Sha256::new();
        h.update(a.verifier.as_bytes());
        assert_eq!(a.challenge, B64.encode(h.finalize()));
    }

    #[test]
    fn build_auth_url_encodes_params_and_scope_is_optional() {
        let pkce = PkceChallenge {
            verifier: "v".into(),
            challenge: "chal".into(),
        };
        let url = build_auth_url(
            "https://as.example/authorize",
            "client-123",
            "http://localhost/cb",
            Some("openid mcp"),
            &pkce,
            "state-xyz",
        )
        .unwrap();
        assert!(url.starts_with("https://as.example/authorize?"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=client-123"));
        assert!(url.contains("code_challenge=chal"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state=state-xyz"));
        // space in scope must be percent-encoded
        assert!(url.contains("scope=openid+mcp") || url.contains("scope=openid%20mcp"));

        let no_scope = build_auth_url(
            "https://as.example/authorize",
            "c",
            "http://localhost/cb",
            None,
            &pkce,
            "s",
        )
        .unwrap();
        assert!(!no_scope.contains("scope="));
    }
}
