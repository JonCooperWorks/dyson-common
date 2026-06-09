//! Typed bearer tokens: `<prefix><32 ASCII-hex>`.
//!
//! A token is a prefix identifying its kind plus a 32-hex-character random
//! body. The newtypes can only be built through [`parse`](ProxyToken::parse)
//! (or serde `Deserialize`, which itself routes through `parse`), so an
//! unvalidated string can never masquerade as a typed token.

/// Number of ASCII-hex characters in a token body.
pub const TOKEN_BODY_HEX_LEN: usize = 32;

pub const PROXY_TOKEN_PREFIX: &str = "pt_";
pub const INGEST_TOKEN_PREFIX: &str = "it_";
pub const STATE_SYNC_TOKEN_PREFIX: &str = "st_";
pub const SESSION_TOKEN_PREFIX: &str = "ses_";

/// Why a string failed to parse as a typed token. Intentionally coarse —
/// the distinction is useful for log forensics, not for an attacker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadToken {
    /// Shorter than the prefix, or a body shorter than 32 chars.
    TooShort,
    /// Body longer than 32 chars.
    TooLong,
    /// Wrong prefix for the kind being parsed.
    WrongPrefix,
    /// Body has the right length but is not 32 hex chars.
    BadBody,
}

impl std::fmt::Display for BadToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TooShort => "token too short",
            Self::TooLong => "token too long",
            Self::WrongPrefix => "token has wrong kind prefix",
            Self::BadBody => "token body is not 32 hex chars",
        })
    }
}

impl std::error::Error for BadToken {}

/// Validate `token` against `prefix` + a 32-hex body. Public so the
/// `typed_token!` macro can reference it from a consuming crate.
pub fn validate(token: &str, prefix: &str) -> Result<(), BadToken> {
    if token.len() < prefix.len() {
        return Err(BadToken::TooShort);
    }
    let Some(rest) = token.strip_prefix(prefix) else {
        return Err(BadToken::WrongPrefix);
    };
    // Report length errors with explicit direction — an over-length body
    // returning "too short" is a contradictory diagnostic.
    match rest.len().cmp(&TOKEN_BODY_HEX_LEN) {
        std::cmp::Ordering::Less => return Err(BadToken::TooShort),
        std::cmp::Ordering::Greater => return Err(BadToken::TooLong),
        std::cmp::Ordering::Equal => {}
    }
    if !rest.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(BadToken::BadBody);
    }
    Ok(())
}

/// Define a prefixed-bearer newtype that can only be built through `parse`.
///
/// Serializes transparently to its wire string; deserializes *through*
/// `parse`, so deserializing an unvalidated string fails rather than minting
/// a malformed typed token.
#[macro_export]
macro_rules! typed_token {
    ($name:ident, $prefix:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, ::serde::Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// The wire prefix for this token kind.
            pub const PREFIX: &'static str = $prefix;

            /// Parse a bearer string into this typed token, enforcing the
            /// prefix and 32-hex body.
            pub fn parse(s: impl Into<String>) -> Result<Self, $crate::tokens::BadToken> {
                let s = s.into();
                $crate::tokens::validate(&s, $prefix)?;
                Ok(Self(s))
            }

            /// Borrow the wire-format string.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consume and return the raw wire string.
            pub fn into_inner(self) -> String {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = <String as ::serde::Deserialize>::deserialize(d)?;
                Self::parse(s).map_err(::serde::de::Error::custom)
            }
        }
    };
}

typed_token!(
    ProxyToken,
    PROXY_TOKEN_PREFIX,
    "Proxy bearer token (`pt_<32hex>`). The agent presents this to the swarm's internal endpoints."
);
typed_token!(
    IngestToken,
    INGEST_TOKEN_PREFIX,
    "Ingest bearer token (`it_<32hex>`) for the state-ingest path."
);
typed_token!(
    StateSyncToken,
    STATE_SYNC_TOKEN_PREFIX,
    "Generation-scoped state-sync token (`st_<32hex>`)."
);
typed_token!(
    SessionToken,
    SESSION_TOKEN_PREFIX,
    "Browser session token (`ses_<32hex>`). Set on the session cookie."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_well_formed() {
        let body = "a".repeat(TOKEN_BODY_HEX_LEN);
        let t = ProxyToken::parse(format!("pt_{body}")).unwrap();
        assert_eq!(t.as_str(), format!("pt_{body}"));
    }

    #[test]
    fn parse_rejects_wrong_prefix_and_bad_shapes() {
        let body = "a".repeat(TOKEN_BODY_HEX_LEN);
        assert_eq!(
            ProxyToken::parse(format!("st_{body}")),
            Err(BadToken::WrongPrefix)
        );
        assert_eq!(
            ProxyToken::parse(format!("pt_{}", "a".repeat(TOKEN_BODY_HEX_LEN - 1))),
            Err(BadToken::TooShort)
        );
        assert_eq!(
            ProxyToken::parse(format!("pt_{}", "a".repeat(TOKEN_BODY_HEX_LEN + 1))),
            Err(BadToken::TooLong)
        );
        assert_eq!(
            ProxyToken::parse(format!("pt_{}", "z".repeat(TOKEN_BODY_HEX_LEN))),
            Err(BadToken::BadBody)
        );
    }

    #[test]
    fn serde_round_trips_and_deserialize_enforces_parse() {
        let body = "0123456789abcdef0123456789abcdef";
        let t = StateSyncToken::parse(format!("st_{body}")).unwrap();
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, format!("\"st_{body}\""));
        let back: StateSyncToken = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);

        // An unvalidated string must NOT deserialize into a typed token.
        assert!(serde_json::from_str::<StateSyncToken>("\"not-a-token\"").is_err());
        assert!(
            serde_json::from_str::<StateSyncToken>("\"pt_0123456789abcdef0123456789abcdef\"")
                .is_err()
        );
    }
}
