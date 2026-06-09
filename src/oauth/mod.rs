//! OAuth 2.0 / MCP-auth wire DTOs (RFC 6749 / 7591 / 8414 / 9728).
//!
//! Used by both `dyson` (agent-side MCP OAuth) and `dyson-swarm` (server-side
//! MCP server auth). The DTOs are always available; the reqwest-bearing flow
//! lives in [`client`] behind the `oauth-client` feature.
//!
//! Fields that had drifted between the two repos are reconciled as supersets:
//! `DcrRequest::scope` (swarm/Smithery needed it) and `TokenResponse::token_type`
//! / `scope` (dyson read `token_type`; swarm dropped both) are all optional, so
//! neither side regresses.

use serde::{Deserialize, Serialize};

/// RFC 8414 authorization-server metadata (the subset we use).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMetadata {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub registration_endpoint: Option<String>,
}

/// RFC 9728 protected-resource metadata. Published by the resource (the MCP
/// server) so a client can find which authorization server(s) issue its tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtectedResourceMetadata {
    #[serde(default)]
    pub authorization_servers: Vec<String>,
}

/// RFC 7591 Dynamic Client Registration request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcrRequest {
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    #[serde(default)]
    pub response_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_endpoint_auth_method: Option<String>,
    /// Space-separated scope list (RFC 7591 §2). Some authorization servers
    /// reject `authorize?scope=foo` unless the client registered with it, so
    /// we mirror requested scopes here. `None` ⇒ omitted; AS picks its default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// RFC 7591 Dynamic Client Registration response (the subset we use).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcrResponse {
    pub client_id: String,
    #[serde(default)]
    pub client_secret: Option<String>,
}

/// RFC 6749 §5.1 token endpoint response. `token_type`/`scope` are optional so
/// servers that omit them and readers that ignore them both work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// A PKCE (RFC 7636) verifier + S256 challenge pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkceChallenge {
    pub verifier: String,
    pub challenge: String,
}

#[cfg(feature = "oauth-client")]
pub mod client;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dcr_request_omits_scope_when_none() {
        let req = DcrRequest {
            client_name: "dyson".into(),
            redirect_uris: vec!["http://localhost/cb".into()],
            grant_types: vec!["authorization_code".into()],
            response_types: vec!["code".into()],
            token_endpoint_auth_method: None,
            scope: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("scope"));
        assert!(!json.contains("token_endpoint_auth_method"));
    }

    #[test]
    fn token_response_tolerates_missing_optional_fields() {
        // A bare access_token response (no token_type/scope/refresh) parses.
        let tr: TokenResponse = serde_json::from_str(r#"{"access_token":"abc"}"#).unwrap();
        assert_eq!(tr.access_token, "abc");
        assert!(tr.token_type.is_none());
        assert!(tr.scope.is_none());

        let full: TokenResponse = serde_json::from_str(
            r#"{"access_token":"abc","token_type":"Bearer","expires_in":3600,"scope":"openid"}"#,
        )
        .unwrap();
        assert_eq!(full.token_type.as_deref(), Some("Bearer"));
        assert_eq!(full.expires_in, Some(3600));
    }
}
