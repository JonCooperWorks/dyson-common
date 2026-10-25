//! The Glitch web-scope compiler, shared by Glitch and Swarm.
//!
//! Glitch only lets an engagement reach targets whose compiled scope matches
//! a scope the trusted control plane (Swarm) compiled independently. Both
//! sides must therefore turn the same declaration into the same digest, byte
//! for byte. Keeping one compiler here, pinned by exact revision in both
//! repositories, makes that agreement structural instead of a hand-synced
//! copy. [`SCOPE_COMPILER_VERSION`] and the golden digest test below change
//! only together, and any change to either is a coordinated release of both
//! consumers (Glitch ADR 0020).
//!
//! The compiler is deliberately pure: it never resolves DNS. The caller
//! supplies `resolved_addresses`, and only the trusted control plane's own
//! answers are ever valid there. Target policy (public-only addresses, host
//! and network caps) belongs to that caller, not to this module.

use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
};

use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

/// Bumped whenever the canonical digest input or normalization changes.
pub const SCOPE_COMPILER_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeDeclaration {
    pub urls: Vec<String>,
    pub hosts: Vec<String>,
    pub cidrs: Vec<IpNet>,
    pub excluded_hosts: Vec<String>,
    /// DNS answers are supplied by the trusted compiler caller and frozen into
    /// this version. Discovery cannot mutate the compiled result.
    pub resolved_addresses: BTreeMap<String, BTreeSet<IpAddr>>,
    pub allowed_ports: BTreeSet<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledScope {
    pub version_id: String,
    pub digest_sha256: String,
    pub hosts: BTreeSet<String>,
    pub addresses: BTreeSet<IpAddr>,
    pub cidrs: BTreeSet<IpNet>,
    pub ports: BTreeSet<u16>,
    pub schemes: BTreeSet<String>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ScopeError {
    #[error("unsupported URL scheme {0}")]
    UnsupportedScheme(String),
    #[error("URL contains credentials")]
    EmbeddedCredentials,
    #[error("URL has no host")]
    MissingHost,
    #[error("host {0} has no frozen DNS answers")]
    MissingDns(String),
    #[error("excluded host {0} is also included")]
    ExcludedHost(String),
    #[error("port {0} is outside the web-only boundary")]
    NonWebPort(u16),
    #[error("scope is empty")]
    Empty,
    #[error("scope serialization failed")]
    Serialization,
}

/// Every host a declaration names, normalized, in the order the compiler
/// sees them: explicit hosts first, then URL hosts. A trusted caller uses
/// this to know which names it must resolve itself.
pub fn declared_hosts(declaration: &ScopeDeclaration) -> Result<BTreeSet<String>, ScopeError> {
    let mut hosts: BTreeSet<String> = declaration
        .hosts
        .iter()
        .map(|h| normalize_host(h))
        .collect();
    for url in &declaration.urls {
        let parsed = Url::parse(url).map_err(|_| ScopeError::MissingHost)?;
        let host = parsed.host_str().ok_or(ScopeError::MissingHost)?;
        hosts.insert(normalize_host(host));
    }
    Ok(hosts)
}

pub fn compile_scope(declaration: &ScopeDeclaration) -> Result<CompiledScope, ScopeError> {
    let exclusions: BTreeSet<_> = declaration
        .excluded_hosts
        .iter()
        .map(|h| normalize_host(h))
        .collect();
    let mut hosts: BTreeSet<String> = declaration
        .hosts
        .iter()
        .map(|h| normalize_host(h))
        .collect();
    let mut schemes = BTreeSet::new();
    let mut ports = declaration.allowed_ports.clone();
    for url in &declaration.urls {
        let parsed = Url::parse(url).map_err(|_| ScopeError::MissingHost)?;
        if !matches!(parsed.scheme(), "http" | "https" | "ws" | "wss") {
            return Err(ScopeError::UnsupportedScheme(parsed.scheme().into()));
        }
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(ScopeError::EmbeddedCredentials);
        }
        let host = parsed.host_str().ok_or(ScopeError::MissingHost)?;
        hosts.insert(normalize_host(host));
        schemes.insert(parsed.scheme().to_owned());
        ports.insert(
            parsed
                .port_or_known_default()
                .ok_or(ScopeError::MissingHost)?,
        );
    }
    if hosts.iter().any(|host| exclusions.contains(host)) {
        let host = hosts
            .iter()
            .find(|host| exclusions.contains(*host))
            .cloned()
            .unwrap_or_default();
        return Err(ScopeError::ExcludedHost(host));
    }
    if ports.iter().any(|port| !matches!(port, 80 | 443)) {
        let port = *ports
            .iter()
            .find(|port| !matches!(port, 80 | 443))
            .unwrap_or(&0);
        return Err(ScopeError::NonWebPort(port));
    }
    let mut addresses = BTreeSet::new();
    for host in &hosts {
        if let Ok(address) = host.parse::<IpAddr>() {
            addresses.insert(address);
            continue;
        }
        let answers = declaration
            .resolved_addresses
            .get(host)
            .ok_or_else(|| ScopeError::MissingDns(host.clone()))?;
        if answers.is_empty() {
            return Err(ScopeError::MissingDns(host.clone()));
        }
        addresses.extend(answers);
    }
    if hosts.is_empty() && declaration.cidrs.is_empty() {
        return Err(ScopeError::Empty);
    }
    let canonical = serde_json::to_vec(&(
        hosts.clone(),
        addresses.clone(),
        declaration.cidrs.clone(),
        ports.clone(),
        schemes.clone(),
    ))
    .map_err(|_| ScopeError::Serialization)?;
    Ok(CompiledScope {
        version_id: uuid::Uuid::new_v4().to_string(),
        digest_sha256: format!("{:x}", Sha256::digest(canonical)),
        hosts,
        addresses,
        cidrs: declaration.cidrs.iter().copied().collect(),
        ports,
        schemes,
    })
}

#[must_use]
pub fn normalize_host(value: &str) -> String {
    value.trim().trim_end_matches('.').to_ascii_lowercase()
}

impl CompiledScope {
    #[must_use]
    pub fn permits_target(&self, url: &Url) -> bool {
        let Some(host) = url.host_str().map(normalize_host) else {
            return false;
        };
        let port = url.port_or_known_default().unwrap_or(0);
        matches!(url.scheme(), "http" | "https" | "ws" | "wss")
            && self.schemes.contains(url.scheme())
            && self.hosts.contains(&host)
            && self.ports.contains(&port)
            && url.username().is_empty()
            && url.password().is_none()
    }

    #[must_use]
    pub fn permits(&self, url: &Url, resolved: IpAddr) -> bool {
        self.permits_target(url)
            && (self.addresses.contains(&resolved)
                || self.cidrs.iter().any(|cidr| cidr.contains(&resolved)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The golden vector. Glitch's broker carries the same declaration and
    /// digest; if this assertion changes, both consumers must move together.
    pub(crate) fn golden() -> ScopeDeclaration {
        ScopeDeclaration {
            urls: vec![
                "https://App.Example.com/login".into(),
                "wss://ws.example.com/socket".into(),
            ],
            hosts: vec!["api.example.com.".into()],
            cidrs: vec![
                "203.0.113.0/28".parse().unwrap(),
                "198.51.100.0/30".parse().unwrap(),
            ],
            excluded_hosts: vec!["admin.example.com".into()],
            resolved_addresses: BTreeMap::from([
                (
                    "app.example.com".into(),
                    BTreeSet::from(["93.184.216.34".parse().unwrap()]),
                ),
                (
                    "ws.example.com".into(),
                    BTreeSet::from([
                        "93.184.216.35".parse().unwrap(),
                        "2606:2800:220:1::1".parse().unwrap(),
                    ]),
                ),
                (
                    "api.example.com".into(),
                    BTreeSet::from(["93.184.216.36".parse().unwrap()]),
                ),
            ]),
            allowed_ports: BTreeSet::from([443]),
        }
    }

    pub(crate) const GOLDEN_DIGEST: &str =
        "fee6b620922d86a21bc602eb3e722180cd93a929e846bfb9e21cd2503b68d9a9";

    #[test]
    fn golden_digest_is_stable() {
        assert_eq!(SCOPE_COMPILER_VERSION, 1);
        let compiled = compile_scope(&golden()).unwrap();
        assert_eq!(compiled.digest_sha256, GOLDEN_DIGEST);
        assert_eq!(
            compiled.hosts,
            BTreeSet::from([
                "api.example.com".to_owned(),
                "app.example.com".to_owned(),
                "ws.example.com".to_owned()
            ])
        );
        assert_eq!(
            compiled.schemes,
            BTreeSet::from(["https".to_owned(), "wss".to_owned()])
        );
        assert_eq!(compiled.ports, BTreeSet::from([443]));
    }

    #[test]
    fn version_ids_are_fresh_but_digests_are_not() {
        let first = compile_scope(&golden()).unwrap();
        let second = compile_scope(&golden()).unwrap();
        assert_ne!(first.version_id, second.version_id);
        assert_eq!(first.digest_sha256, second.digest_sha256);
    }

    #[test]
    fn declared_hosts_lists_every_name_to_resolve() {
        assert_eq!(
            declared_hosts(&golden()).unwrap(),
            BTreeSet::from([
                "api.example.com".to_owned(),
                "app.example.com".to_owned(),
                "ws.example.com".to_owned()
            ])
        );
    }

    #[test]
    fn refusals_fail_closed() {
        let mut value = golden();
        value.urls = vec!["ftp://app.example.com".into()];
        assert_eq!(
            compile_scope(&value),
            Err(ScopeError::UnsupportedScheme("ftp".into()))
        );
        let mut value = golden();
        value.urls = vec!["https://user:secret@app.example.com".into()];
        assert_eq!(compile_scope(&value), Err(ScopeError::EmbeddedCredentials));
        let mut value = golden();
        value.allowed_ports.insert(22);
        assert_eq!(compile_scope(&value), Err(ScopeError::NonWebPort(22)));
        let mut value = golden();
        value.urls = vec!["https://app.example.com:8443/".into()];
        assert_eq!(compile_scope(&value), Err(ScopeError::NonWebPort(8443)));
        let mut value = golden();
        value.resolved_addresses.remove("api.example.com");
        assert_eq!(
            compile_scope(&value),
            Err(ScopeError::MissingDns("api.example.com".into()))
        );
        let mut value = golden();
        value.excluded_hosts = vec!["API.example.com".into()];
        assert_eq!(
            compile_scope(&value),
            Err(ScopeError::ExcludedHost("api.example.com".into()))
        );
        let empty = ScopeDeclaration {
            urls: vec![],
            hosts: vec![],
            cidrs: vec![],
            excluded_hosts: vec![],
            resolved_addresses: BTreeMap::new(),
            allowed_ports: BTreeSet::new(),
        };
        assert_eq!(compile_scope(&empty), Err(ScopeError::Empty));
    }

    #[test]
    fn permits_requires_scheme_host_port_and_frozen_address() {
        let scope = compile_scope(&golden()).unwrap();
        let url = Url::parse("https://app.example.com/x").unwrap();
        assert!(scope.permits(&url, "93.184.216.34".parse().unwrap()));
        assert!(!scope.permits(&url, "93.184.216.99".parse().unwrap()));
        assert!(scope.permits(&url, "203.0.113.5".parse().unwrap()));
        assert!(!scope.permits(
            &Url::parse("http://app.example.com/x").unwrap(),
            "93.184.216.34".parse().unwrap()
        ));
        assert!(!scope.permits(
            &Url::parse("https://other.example.com/").unwrap(),
            "93.184.216.34".parse().unwrap()
        ));
    }
}
