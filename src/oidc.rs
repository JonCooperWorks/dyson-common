//! OIDC JWKS wire types + key construction. Behind the `oidc` feature.
//!
//! Both repos validate inbound OIDC JWTs (dyson as an MCP server, swarm as the
//! SPA backend). The JWKS *fetching*, *caching*, and claim *policy* differ and
//! stay in each repo; what's shared here is the RFC 7517 [`Jwk`]/[`JwkSet`] wire
//! shape and [`decoding_key_from_jwk`] — the RSA/EC/HMAC key construction that
//! was duplicated between them.

use jsonwebtoken::DecodingKey;
use serde::Deserialize;

/// A single JSON Web Key (RFC 7517) — only the fields we verify with.
///
/// `alg`/`kty`/key-material default to `None` so an unfamiliar key can be
/// skipped (a partially-understood JWKS shouldn't fail the whole set).
#[derive(Debug, Clone, Deserialize)]
pub struct Jwk {
    pub kid: String,
    #[serde(default)]
    pub alg: Option<String>,
    #[serde(default)]
    pub kty: Option<String>,
    // RSA
    #[serde(default)]
    pub n: Option<String>,
    #[serde(default)]
    pub e: Option<String>,
    // EC (the curve is inferred from the algorithm)
    #[serde(default)]
    pub x: Option<String>,
    #[serde(default)]
    pub y: Option<String>,
    // HMAC (uncommon for OIDC issuers; supported for local dev IdPs)
    #[serde(default)]
    pub k: Option<String>,
}

/// A set of JWKs as returned from a provider's `jwks_uri`.
#[derive(Debug, Clone, Deserialize)]
pub struct JwkSet {
    pub keys: Vec<Jwk>,
}

impl JwkSet {
    /// Find a key by its `kid`.
    pub fn find(&self, kid: &str) -> Option<&Jwk> {
        self.keys.iter().find(|k| k.kid == kid)
    }
}

/// Build a [`DecodingKey`] from a [`Jwk`], choosing the builder by the JWK's
/// `alg` (if the IdP pins it) then falling back to `kty`.
///
/// Returns `None` for a key whose type/material we don't understand or can't
/// construct — callers treat that as a key miss (→ 401), so a partially-
/// understood JWKS doesn't brick verification.
pub fn decoding_key_from_jwk(key: &Jwk) -> Option<DecodingKey> {
    let alg = key.alg.as_deref();
    let kty = key.kty.as_deref();
    match (alg, kty) {
        (Some("RS256" | "RS384" | "RS512"), _) | (_, Some("RSA")) => {
            DecodingKey::from_rsa_components(key.n.as_deref()?, key.e.as_deref()?).ok()
        }
        (Some("ES256" | "ES384" | "ES512"), _) | (_, Some("EC")) => {
            DecodingKey::from_ec_components(key.x.as_deref()?, key.y.as_deref()?).ok()
        }
        (Some("HS256" | "HS384" | "HS512"), _) | (_, Some("oct")) => {
            DecodingKey::from_base64_secret(key.k.as_deref()?).ok()
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jwks_and_finds_by_kid() {
        let set: JwkSet = serde_json::from_str(
            r#"{"keys":[{"kid":"a","kty":"RSA","n":"x","e":"AQAB"},{"kid":"b","kty":"oct"}]}"#,
        )
        .unwrap();
        assert!(set.find("a").is_some());
        assert!(set.find("b").is_some());
        assert!(set.find("missing").is_none());
    }

    #[test]
    fn unknown_key_type_is_a_miss_not_a_panic() {
        let jwk = Jwk {
            kid: "x".into(),
            alg: None,
            kty: Some("unknown".into()),
            n: None,
            e: None,
            x: None,
            y: None,
            k: None,
        };
        assert!(decoding_key_from_jwk(&jwk).is_none());
    }

    #[test]
    fn hmac_key_builds() {
        let jwk = Jwk {
            kid: "h".into(),
            alg: Some("HS256".into()),
            kty: Some("oct".into()),
            n: None,
            e: None,
            x: None,
            y: None,
            k: Some("c2VjcmV0".into()), // base64("secret")
        };
        assert!(decoding_key_from_jwk(&jwk).is_some());
    }
}
