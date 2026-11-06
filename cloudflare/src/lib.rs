//! Cloudflare for Dyson products: zones a token reaches, DNS records, zone
//! rules kept by `ref`, and Workers AI account URLs.
//!
//! This crate talks to Cloudflare and nothing else. Products own where tokens
//! are stored, which records they made, who may touch which zone, scheduling
//! and every policy about what a name should point at. Nothing here reads the
//! environment or keeps state between calls.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub mod ai;
mod client;
pub mod reconcile;

pub use client::{Client, DEFAULT_API};

/// What went wrong talking to Cloudflare, or with what was asked of it.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No answer: DNS, TLS, timeout, connection.
    #[error("Cloudflare is unreachable: {0}")]
    Unreachable(String),
    /// Cloudflare answered and refused, with the codes and messages of its
    /// `{ errors: [{ code, message }] }` envelope.
    #[error("Cloudflare answered {status}{}", said(messages))]
    Api {
        status: u16,
        codes: Vec<i64>,
        messages: Vec<String>,
    },
    /// Cloudflare does not know the zone, record or ruleset (HTTP 404).
    #[error("Cloudflare does not know it")]
    NotFound,
    /// The name already has records at Cloudflare: the asker's to fix.
    #[error("{0}")]
    Taken(String),
    /// The input was refused before Cloudflare was asked.
    #[error("{0}")]
    Invalid(String),
    /// Cloudflare answered success with a body this crate cannot read.
    #[error("Cloudflare answered something unexpected: {0}")]
    Malformed(String),
}

fn said(messages: &[String]) -> String {
    if messages.is_empty() {
        String::new()
    } else {
        format!(": {}", messages.join("; "))
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A Cloudflare API token. It is wiped on drop, and `Debug` shows only
/// [`hint`].
#[derive(Clone)]
pub struct Token(zeroize::Zeroizing<String>);

impl Token {
    /// Trims the value and refuses anything that cannot be a token.
    pub fn new(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            return Err(Error::Invalid("the token is required".into()));
        }
        if value.len() > 256 || !value.chars().all(|c| c.is_ascii_graphic()) {
            return Err(Error::Invalid(
                "that does not look like a Cloudflare API token".into(),
            ));
        }
        Ok(Self(zeroize::Zeroizing::new(value.to_owned())))
    }

    /// The token's value, for a product that must seal or compare it.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn hint(&self) -> String {
        hint(&self.0)
    }
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Token({})", self.hint())
    }
}

/// The last four characters of a token, so a person can tell which it is.
pub fn hint(token: &str) -> String {
    let tail: Vec<char> = token.chars().rev().take(4).collect();
    let tail: String = tail.into_iter().rev().collect();
    format!("…{tail}")
}

/// What `tokens/verify` says about a token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenStatus {
    Active,
    /// `disabled`, `expired`, … as Cloudflare names it.
    Other(String),
}

/// A zone a token reaches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Zone {
    pub id: String,
    /// Lowercased.
    pub name: String,
    /// `active`, `pending`, …: only an active zone answers.
    pub status: String,
    pub account_id: Option<String>,
    pub account_name: Option<String>,
}

/// The record types this crate makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordKind {
    A,
    #[allow(clippy::upper_case_acronyms)]
    AAAA,
    #[allow(clippy::upper_case_acronyms)]
    CNAME,
}

impl RecordKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::AAAA => "AAAA",
            Self::CNAME => "CNAME",
        }
    }

    pub fn of_ip(ip: std::net::IpAddr) -> Self {
        if ip.is_ipv4() { Self::A } else { Self::AAAA }
    }
}

/// A DNS record as Cloudflare lists it. `kind` is kept as Cloudflare's string
/// so records of types this crate does not make can still be seen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub content: String,
    pub proxied: bool,
    pub ttl: u32,
}

impl Record {
    pub fn is_address(&self) -> bool {
        self.kind == "A" || self.kind == "AAAA"
    }
}

/// A record to create, or what to change one to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NewRecord {
    #[serde(rename = "type")]
    pub kind: RecordKind,
    pub name: String,
    pub content: String,
    pub proxied: bool,
    /// Seconds; `1` is Cloudflare's "automatic" (required when proxied).
    pub ttl: u32,
}

impl NewRecord {
    pub fn address(name: &str, ip: std::net::IpAddr, proxied: bool) -> Self {
        Self {
            kind: RecordKind::of_ip(ip),
            name: name.to_owned(),
            content: ip.to_string(),
            proxied,
            ttl: 1,
        }
    }

    pub fn cname(name: &str, target: &str, proxied: bool) -> Self {
        Self {
            kind: RecordKind::CNAME,
            name: name.to_owned(),
            content: target.trim_end_matches('.').to_ascii_lowercase(),
            proxied,
            ttl: 1,
        }
    }
}

/// A zone-level managed header and whether it is on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManagedHeader {
    pub id: String,
    pub enabled: bool,
}

/// Whether a call changed anything at Cloudflare.
pub type Changed = bool;

fn valid_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A bare hostname with at least two labels, lowercased, without a trailing
/// dot. A scheme, path, port or wildcard is refused.
pub fn normalize_host(value: &str) -> Result<String> {
    let host = value.trim().trim_end_matches('.').to_ascii_lowercase();
    let invalid = |why: &str| Err(Error::Invalid(why.into()));
    if host.is_empty() {
        return invalid("the domain is required");
    }
    if host.contains("://") || host.contains('/') || host.contains(':') {
        return invalid("enter a bare hostname like api.example.com, not a URL");
    }
    if host.len() > 253 {
        return invalid("the domain is too long");
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 || !labels.iter().all(|l| valid_label(l)) {
        return invalid("the domain must look like api.example.com");
    }
    if !labels
        .last()
        .is_some_and(|tld| tld.len() >= 2 && !tld.bytes().all(|b| b.is_ascii_digit()))
    {
        return invalid("the domain needs a real top-level domain");
    }
    Ok(host)
}

/// The zone a host belongs to: the longest zone name that is the host or a
/// parent of it. Both sides compare lowercased.
pub fn zone_for<'a>(host: &str, zones: &'a [Zone]) -> Option<&'a Zone> {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    zones
        .iter()
        .filter(|zone| {
            let name = zone.name.to_ascii_lowercase();
            host == name || host.ends_with(&format!(".{name}"))
        })
        .max_by_key(|zone| zone.name.len())
}

/// A rules-language expression matching exactly these hosts:
/// `(http.host in {"a.example.com" "b.example.com"})`. Quotes and
/// backslashes are escaped, so a host can never end the string early.
pub fn hosts_expression(hosts: &BTreeSet<String>) -> String {
    let names: Vec<String> = hosts
        .iter()
        .map(|host| format!("\"{}\"", host.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    format!("(http.host in {{{}}})", names.join(" "))
}

/// Whether a stored rule already says what `wanted` does. Fields Cloudflare
/// adds (`id`, `version`, `last_updated`, …) are ignored.
pub fn same_rule(have: &serde_json::Value, wanted: &serde_json::Value) -> bool {
    ["expression", "action", "action_parameters", "enabled"]
        .iter()
        .all(|field| have.get(field) == wanted.get(field))
}

/// A Cloudflare id (zone, record) or phase name safe to put in a path.
fn path_part<'a>(what: &str, value: &'a str) -> Result<&'a str> {
    if !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        Ok(value)
    } else {
        Err(Error::Invalid(format!("{what} is not a Cloudflare id")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn zone(name: &str) -> Zone {
        Zone {
            id: format!("z-{name}"),
            name: name.into(),
            status: "active".into(),
            account_id: None,
            account_name: None,
        }
    }

    #[test]
    fn a_token_is_shown_by_its_last_four() {
        assert_eq!(hint("abcdefgh1234"), "…1234");
        assert_eq!(hint("ab"), "…ab");
        let token = Token::new("  secret-token-9876 ").unwrap();
        assert_eq!(token.expose(), "secret-token-9876");
        assert_eq!(format!("{token:?}"), "Token(…9876)");
        assert!(Token::new("").is_err());
        assert!(Token::new("has space").is_err());
        assert!(Token::new(&"x".repeat(257)).is_err());
    }

    #[test]
    fn hosts_are_bare_lowercase_names() {
        assert_eq!(
            normalize_host(" API.Example.com. ").unwrap(),
            "api.example.com"
        );
        for bad in [
            "",
            "https://api.example.com",
            "api.example.com/x",
            "api.example.com:443",
            "localhost",
            "*.example.com",
            "-a.example.com",
            "a.example.123",
            "a_b.example.com",
        ] {
            assert!(normalize_host(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_longest_zone_wins() {
        let zones = vec![
            zone("example.com"),
            zone("dev.example.com"),
            zone("other.org"),
        ];
        assert_eq!(
            zone_for("api.dev.example.com", &zones).unwrap().name,
            "dev.example.com"
        );
        assert_eq!(
            zone_for("API.example.com.", &zones).unwrap().name,
            "example.com"
        );
        assert_eq!(zone_for("example.com", &zones).unwrap().name, "example.com");
        assert!(zone_for("notexample.com", &zones).is_none());
        assert!(zone_for("example.org", &zones).is_none());
    }

    #[test]
    fn the_expression_names_exactly_the_hosts_and_cannot_be_broken_out_of() {
        let hosts: BTreeSet<String> = ["b.example.com".to_owned(), "a.example.com".to_owned()]
            .into_iter()
            .collect();
        assert_eq!(
            hosts_expression(&hosts),
            r#"(http.host in {"a.example.com" "b.example.com"})"#
        );
        let hostile: BTreeSet<String> = [r#"a" or true or "\"#.to_owned()].into_iter().collect();
        assert_eq!(
            hosts_expression(&hostile),
            r#"(http.host in {"a\" or true or \"\\"})"#
        );
    }

    #[test]
    fn a_rule_is_the_same_whatever_cloudflare_added() {
        let wanted = json!({
            "ref": "r", "expression": "(http.host in {\"a.example.com\"})",
            "action": "set_config", "action_parameters": {"ssl": "full"}, "enabled": true,
        });
        let mut stored = wanted.clone();
        stored["id"] = json!("abc");
        stored["version"] = json!("3");
        stored["last_updated"] = json!("2026-11-06T00:00:00Z");
        assert!(same_rule(&stored, &wanted));
        stored["expression"] = json!("(http.host in {\"b.example.com\"})");
        assert!(!same_rule(&stored, &wanted));
    }

    #[test]
    fn path_parts_are_ids_only() {
        assert!(path_part("zone", "023e105f4ecef8ad9ca31a8372d0c353").is_ok());
        assert!(path_part("phase", "http_config_settings").is_ok());
        for bad in ["", "../x", "a/b", "a?b", "a b", &"x".repeat(65)] {
            assert!(path_part("zone", bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn errors_read_like_the_honeypot_s() {
        let error = Error::Api {
            status: 403,
            codes: vec![9109],
            messages: vec!["Invalid access token".into()],
        };
        assert_eq!(
            error.to_string(),
            "Cloudflare answered 403: Invalid access token"
        );
        let bare = Error::Api {
            status: 500,
            codes: vec![],
            messages: vec![],
        };
        assert_eq!(bare.to_string(), "Cloudflare answered 500");
    }
}
