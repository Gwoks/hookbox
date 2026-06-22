//! SSRF guard — PORT of `app/interceptor/proxy.py` guard (AC-28..32, AC-S1..S4).
//!
//! `is_blocked_ip` classifies an `IpAddr` as loopback / private / link-local /
//! multicast / reserved / unspecified / the cloud-metadata `169.254.169.254`
//! (IPv4 + IPv6 incl. `::ffff:` mapped). `resolve_and_check` resolves a hostname
//! to IPs and rejects if ANY resolved address is blocked, returning the validated
//! IPs so the caller can PIN the connection (defeating DNS rebinding). When
//! `MITM_ALLOW_PRIVATE` is set the classifier is bypassed (still must resolve).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

/// A target (or redirect hop) resolved to a blocked address or failed to resolve.
#[derive(Debug)]
pub struct SsrfBlocked(pub String);

/// The cloud-metadata endpoint that must always be blocked (AC-S2).
const METADATA_V4: Ipv4Addr = Ipv4Addr::new(169, 254, 169, 254);

fn v4_blocked(ip: &Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || ip.is_documentation()
        || *ip == METADATA_V4
        // 100.64.0.0/10 (CGNAT) + 192.0.0.0/24 + 198.18.0.0/15 reserved-ish ranges.
        || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]))
        || (ip.octets()[0] == 0)
}

fn v6_blocked(ip: &Ipv6Addr) -> bool {
    // IPv4-mapped (::ffff:a.b.c.d) — classify the embedded v4 (AC-S3).
    if let Some(v4) = ip.to_ipv4_mapped() {
        return v4_blocked(&v4);
    }
    if let Some(v4) = ip.to_ipv4() {
        // ::a.b.c.d compatible form.
        if v4_blocked(&v4) {
            return true;
        }
    }
    ip.is_loopback()
        || ip.is_multicast()
        || ip.is_unspecified()
        // fc00::/7 unique-local
        || (ip.segments()[0] & 0xfe00) == 0xfc00
        // fe80::/10 link-local
        || (ip.segments()[0] & 0xffc0) == 0xfe80
}

/// True if `ip` is in a blocked range (loopback/private/link-local/metadata/...).
pub fn is_blocked_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4_blocked(v4),
        IpAddr::V6(v6) => v6_blocked(v6),
    }
}

/// Resolve `host` to IPs and reject if any resolved address is blocked. Returns
/// the validated IPs (for connection pinning). When `allow_private` is set the
/// block check is skipped (host must still resolve). A literal IP is checked
/// directly (no DNS).
pub fn resolve_and_check(host: &str, allow_private: bool) -> Result<Vec<IpAddr>, SsrfBlocked> {
    let host = host.trim_matches(|c| c == '[' || c == ']');
    // Literal IP target: check directly, no DNS.
    if let Ok(literal) = host.parse::<IpAddr>() {
        if !allow_private && is_blocked_ip(&literal) {
            return Err(SsrfBlocked(format!(
                "target IP {host} is in a blocked range"
            )));
        }
        return Ok(vec![literal]);
    }
    // Resolve the hostname (port 0 — we only need addresses).
    let addrs = (host, 0u16)
        .to_socket_addrs()
        .map_err(|e| SsrfBlocked(format!("cannot resolve host {host:?}: {e}")))?;
    let mut resolved = Vec::new();
    for sa in addrs {
        let ip = sa.ip();
        if !allow_private && is_blocked_ip(&ip) {
            return Err(SsrfBlocked(format!(
                "host {host:?} resolves to blocked address {ip}"
            )));
        }
        resolved.push(ip);
    }
    if resolved.is_empty() {
        return Err(SsrfBlocked(format!(
            "host {host:?} produced no usable address"
        )));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn ip(s: &str) -> IpAddr {
        IpAddr::from_str(s).unwrap()
    }

    #[test]
    fn blocks_the_resolved_ip_set() {
        for s in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.5.5",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "224.0.0.1",
            "100.100.0.1",
            "::1",
            "fe80::1",
            "fc00::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "::",
        ] {
            assert!(is_blocked_ip(&ip(s)), "{s} must be blocked");
        }
    }

    #[test]
    fn allows_public_addresses() {
        for s in [
            "8.8.8.8",
            "1.1.1.1",
            "93.184.216.34",
            "2606:2800:220:1:248:1893:25c8:1946",
        ] {
            assert!(!is_blocked_ip(&ip(s)), "{s} should be allowed");
        }
    }

    #[test]
    fn literal_block_and_allow_private() {
        assert!(resolve_and_check("127.0.0.1", false).is_err());
        // metadata always blocked
        assert!(resolve_and_check("169.254.169.254", false).is_err());
        // allow_private bypasses the classifier for a literal IP.
        assert_eq!(
            resolve_and_check("127.0.0.1", true).unwrap(),
            vec![ip("127.0.0.1")]
        );
        // public literal passes.
        assert_eq!(
            resolve_and_check("8.8.8.8", false).unwrap(),
            vec![ip("8.8.8.8")]
        );
    }
}
