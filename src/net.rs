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

/// Whether an address is ordinary public unicast that an operator-declared
/// security test may target. Strictly narrower than `!is_private_*`: it also
/// refuses documentation, benchmarking and IETF-reserved ranges, site-local
/// IPv6, and every IPv6 form that embeds an IPv4 address (mapped, NAT64,
/// 6to4, Teredo, IPv4-compatible), because the embedded address cannot be
/// policed reliably at the edge.
pub fn is_public_target(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let o = v4.octets();
            !(is_private_v4(v4)
                || v4.is_documentation()
                // 192.0.0.0/24 IETF protocol assignments
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)
                // 192.88.99.0/24 deprecated 6to4 relay anycast
                || (o[0] == 192 && o[1] == 88 && o[2] == 99)
                // 198.18.0.0/15 benchmarking
                || (o[0] == 198 && (o[1] & 0xfe) == 18))
        }
        std::net::IpAddr::V6(v6) => {
            let seg = v6.segments();
            !(is_private_v6(v6)
                || v6.to_ipv4_mapped().is_some()
                // IPv4-compatible ::a.b.c.d (deprecated)
                || (seg[..6] == [0, 0, 0, 0, 0, 0])
                // 64:ff9b::/96 and 64:ff9b:1::/48 NAT64
                || (seg[0] == 0x64 && seg[1] == 0xff9b)
                // 2002::/16 6to4
                || seg[0] == 0x2002
                // 2001::/32 Teredo, 2001:db8::/32 documentation,
                // 2001:2::/48 benchmarking, 2001:10::/28 ORCHID
                || (seg[0] == 0x2001 && (seg[1] == 0 || seg[1] == 0x0db8 || seg[1] == 2 || (seg[1] & 0xfff0) == 0x10))
                // 100::/64 discard
                || (seg[0] == 0x100 && seg[1..4] == [0, 0, 0])
                // fec0::/10 deprecated site-local
                || (seg[0] & 0xffc0) == 0xfec0
                // 3fff::/20 documentation (RFC 9637)
                || (seg[0] == 0x3fff && (seg[1] & 0xf000) == 0))
        }
    }
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
///
/// The URL is parsed the way reqwest parses it (WHATWG, via the `url`
/// crate), so the host returned is the host a client connects to:
/// `http://127.0.0.1?@example.com` is `127.0.0.1`, not `example.com`.
/// The host is returned as written in `url` (its ASCII case kept), and
/// only when what is written *is* that host. When the parser had to
/// rewrite it to get there (percent-encoding, IDNA, tabs or newlines, the
/// IPv4 shorthands `127.1` and `0x7f000001`, a backslash before the
/// authority), the answer is `None`, as it is for no `://`, no host, an
/// empty host or a URL that does not parse: callers deciding whether a
/// request may go treat `None` as "refuse".
pub fn host_from_url(url: &str) -> Option<&str> {
    let parsed = url::Url::parse(url).ok()?;
    let after_scheme = &url[url.find("://")? + 3..];
    // The authority ends where WHATWG ends it for the special schemes:
    // at `/`, `\`, `?` or `#`. For any other scheme a `\` belongs to the
    // authority, and the check against the parser below refuses it.
    let authority = after_scheme
        .split(['/', '\\', '?', '#'])
        .next()
        .unwrap_or(after_scheme);
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    let written = if let Some(bracketed) = host_port.strip_prefix('[') {
        &bracketed[..bracketed.find(']')?]
    } else {
        host_port.split(':').next().unwrap_or(host_port)
    };
    if written.is_empty() {
        return None;
    }
    let same = match parsed.host()? {
        url::Host::Domain(domain) => written.eq_ignore_ascii_case(domain),
        url::Host::Ipv4(ip) => written.parse::<Ipv4Addr>() == Ok(ip),
        url::Host::Ipv6(ip) => written.parse::<Ipv6Addr>() == Ok(ip),
    };
    same.then_some(written)
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

    /// The host is the one a WHATWG client (reqwest) connects to, never a
    /// string a delimiter the old splitter ignored put after an `@`.
    #[test]
    fn host_from_url_agrees_with_whatwg() {
        for (url, host) in [
            // Query and fragment end the authority before the `@`.
            ("http://127.0.0.1?@example.com", Some("127.0.0.1")),
            ("http://127.0.0.1#@example.com", Some("127.0.0.1")),
            ("http://127.0.0.1?x=1@example.com/", Some("127.0.0.1")),
            // So does a backslash, for http(s).
            ("http://127.0.0.1\\@example.com", Some("127.0.0.1")),
            (
                "https://example.com\\@169.254.169.254/",
                Some("example.com"),
            ),
            // Userinfo, with and without a password, and an `@` inside it.
            ("http://user@example.com/", Some("example.com")),
            ("http://u:p@example.com:8080/", Some("example.com")),
            ("http://a@b@example.com/", Some("example.com")),
            ("http://example.com@127.0.0.1/", Some("127.0.0.1")),
            // IPv6, with and without a port.
            ("http://[::1]/", Some("::1")),
            ("http://[fe80::1]:8080/x", Some("fe80::1")),
            ("http://u@[::ffff:127.0.0.1]:1/", Some("::ffff:127.0.0.1")),
            // Ports, and case kept as written.
            ("https://Example.COM:8443", Some("Example.COM")),
            ("http://10.0.0.1:80?q", Some("10.0.0.1")),
            // Empty or missing hosts, and no scheme.
            ("http://", None),
            ("http:///x", None),
            ("http://@/x", None),
            ("http://:80/", None),
            ("http://[]/", None),
            ("example.com/path", None),
            ("//example.com/", None),
            ("", None),
            // Rewritten by the parser: refused rather than guessed.
            ("http://0x7f000001/", None),
            ("http://127.1/", None),
            ("http://2130706433/", None),
            ("http://%31%32%37.0.0.1/", None),
            ("http://exa\tmple.com/", None),
            ("http://\\127.0.0.1/", None),
            (
                "http://xn--bcher-kva.example/",
                Some("xn--bcher-kva.example"),
            ),
            ("http://bücher.example/", None),
            // A bad port is not a URL at all.
            ("http://example.com:99999/", None),
        ] {
            assert_eq!(host_from_url(url), host, "{url:?}");
        }
    }

    #[test]
    fn public_target_refuses_every_internal_or_reserved_form() {
        use std::net::IpAddr;
        for ip in [
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "127.0.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "255.255.255.255",
            "224.0.0.1",
            "240.0.0.1",
            "192.0.2.10",
            "198.51.100.1",
            "203.0.113.9",
            "192.0.0.8",
            "198.18.0.1",
            "192.88.99.1",
            "::1",
            "::",
            "fe80::1",
            "fd00::1",
            "fec0::1",
            "ff02::1",
            "::ffff:10.0.0.1",
            "::ffff:8.8.8.8",
            "::8.8.8.8",
            "64:ff9b::a00:1",
            "2002:a00:1::1",
            "2001::1",
            "2001:db8::1",
            "2001:2::1",
            "2001:10::1",
            "100::1",
            "3fff::1",
        ] {
            assert!(!is_public_target(ip.parse::<IpAddr>().unwrap()), "{ip}");
        }
        for ip in [
            "93.184.216.34",
            "8.8.8.8",
            "1.1.1.1",
            "2606:4700:4700::1111",
            "2a00:1450:4001::1",
        ] {
            assert!(is_public_target(ip.parse::<IpAddr>().unwrap()), "{ip}");
        }
    }
}
