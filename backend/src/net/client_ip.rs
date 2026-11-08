//! Client-IP resolution for the per-source rate limiters (session + public
//! share resolver).

use std::net::SocketAddr;

use axum::extract::ConnectInfo;
use axum::http::HeaderMap;

/// Resolve the rate-limit key for a request. Behind the deploy's nginx, every
/// TCP peer is loopback (`proxy_pass http://127.0.0.1:{port}`), which would
/// otherwise collapse every client onto one shared bucket. When the peer is
/// loopback, trust the proxy's `X-Real-IP` (falling back to the first
/// `X-Forwarded-For` hop) so the anti-enumeration limit (AC-S5) is actually
/// per-source; a non-loopback peer can't be nginx, so its headers are never
/// trusted and the real socket peer wins.
pub(crate) fn effective_client_ip(
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: &HeaderMap,
) -> String {
    let peer = connect_info.map(|ci| ci.0.ip());
    if peer.map(|ip| ip.is_loopback()).unwrap_or(false) {
        let real_ip = headers
            .get("x-real-ip")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if let Some(ip) = real_ip {
            return ip.to_string();
        }
        let forwarded = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if let Some(ip) = forwarded {
            return ip.to_string();
        }
    }
    peer.map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(ip: &str) -> Option<ConnectInfo<SocketAddr>> {
        let addr: std::net::IpAddr = ip.parse().unwrap();
        Some(ConnectInfo(SocketAddr::new(addr, 0)))
    }

    #[test]
    fn loopback_peer_trusts_proxy_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "203.0.113.7".parse().unwrap());
        assert_eq!(
            effective_client_ip(conn("127.0.0.1"), &headers),
            "203.0.113.7"
        );
    }

    #[test]
    fn loopback_peer_falls_back_to_forwarded_for_first_hop() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1".parse().unwrap());
        assert_eq!(effective_client_ip(conn("::1"), &headers), "203.0.113.7");
    }

    #[test]
    fn loopback_peer_without_headers_keeps_loopback() {
        assert_eq!(
            effective_client_ip(conn("127.0.0.1"), &HeaderMap::new()),
            "127.0.0.1"
        );
    }

    #[test]
    fn non_loopback_peer_ignores_spoofed_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "203.0.113.7".parse().unwrap());
        assert_eq!(
            effective_client_ip(conn("198.51.100.9"), &headers),
            "198.51.100.9"
        );
    }

    #[test]
    fn no_connect_info_is_unknown() {
        assert_eq!(effective_client_ip(None, &HeaderMap::new()), "unknown");
    }
}
