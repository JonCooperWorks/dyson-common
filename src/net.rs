//! SSRF / internal-network primitives shared by both repos' HTTP policies.
//!
//! These are the pure predicates — "is this IP internal?", "is this a cloud
//! metadata host?", "what's the host of this URL?".  Each repo keeps its own
//! *client* and *policy* (dyson's redirect-guarded singleton, swarm's per-fetch
//! IP-pinned external client + egress CIDR engine) but builds them on this one
//! definition of "internal", so the two can't drift on what they block.
//!
//! `is_private_v4`/`is_private_v6` are the union of both repos' prior
//! hand-rolled checks (RFC1918, loopback, link-local, broadcast, multicast,
//! unspecified, RFC 6598 CGNAT, 0.0.0.0/8, class-E 240/4, ULA, v4-mapped v6).

use std::net::{Ipv4Addr, Ipv6Addr};

/// Whether an IPv4 address is in a range no outbound request should reach.
pub fn is_private_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || ip.is_multicast()
        // RFC 6598 shared address space (CGNAT): 100.64.0.0/10
        || (o[0] == 100 && (o[1] & 0xc0) == 64)
        // 0.0.0.0/8 ("this network")
        || o[0] == 0
        // Class E / reserved: 240.0.0.0/4 (incl. 255.255.255.255 broadcast)
        || o[0] >= 240
}

/// Companion to [`is_private_v4`] for IPv6 (incl. IPv4-mapped addresses).
pub fn is_private_v6(ip: Ipv6Addr) -> bool {
    let seg = ip.segments();
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        // Unique local addresses: fc00::/7
        || (seg[0] & 0xfe00) == 0xfc00
        // Link-local: fe80::/10
        || (seg[0] & 0xffc0) == 0xfe80
        // IPv4-mapped IPv6 → check the embedded v4.
        || matches!(ip.to_ipv4_mapped(), Some(v4) if is_private_v4(v4))
}

/// Whether a hostname matches a well-known cloud metadata service.
pub fn is_metadata_host(host: &str) -> bool {
    let h = host.trim_end_matches('.').to_ascii_lowercase();
    matches!(
        h.as_str(),
        "localhost"
            | "metadata.google.internal"
            | "metadata"
            | "metadata.aws.amazon.com"
            | "metadata.azure.com"
            | "metadata.tencentyun.com"
            | "metadata.packet.net"
    )
}

/// Extract the host from a URL: strips scheme, userinfo, path, query,
/// fragment, and port; unwraps IPv6 brackets (`[::1]:8080` → `::1`).
/// Returns `None` if there's no `://` scheme or the host is empty.
pub fn host_from_url(url: &str) -> Option<&str> {
    let after_scheme = &url[url.find("://")? + 3..];
    let authority = after_scheme.split('/').next().unwrap_or(after_scheme);
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    let host = if host_port.starts_with('[') {
        host_port
            .find(']')
            .map(|i| &host_port[1..i])
            .unwrap_or(host_port)
    } else {
        host_port.split(':').next().unwrap_or(host_port)
    };
    if host.is_empty() { None } else { Some(host) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v4_blocks_internal_and_reserved_ranges() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "240.0.0.1",
            "255.255.255.255",
        ] {
            assert!(
                is_private_v4(ip.parse().unwrap()),
                "{ip} should be internal"
            );
        }
        for ip in ["8.8.8.8", "1.1.1.1", "93.184.216.34"] {
            assert!(!is_private_v4(ip.parse().unwrap()), "{ip} should be public");
        }
    }

    #[test]
    fn v6_blocks_internal_incl_v4_mapped() {
        for ip in [
            "::1",
            "::",
            "fe80::1",
            "fc00::1",
            "ff02::1",
            "::ffff:10.0.0.1",
        ] {
            assert!(
                is_private_v6(ip.parse().unwrap()),
                "{ip} should be internal"
            );
        }
        assert!(!is_private_v6("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn metadata_hosts_and_host_extraction() {
        assert!(is_metadata_host("metadata.google.internal"));
        assert!(is_metadata_host("METADATA.GOOGLE.INTERNAL."));
        assert!(!is_metadata_host("example.com"));

        assert_eq!(
            host_from_url("https://user:pw@example.com:8443/p?q=1"),
            Some("example.com")
        );
        assert_eq!(host_from_url("http://[::1]:8080/x"), Some("::1"));
        assert_eq!(host_from_url("no-scheme/path"), None);
    }
}
