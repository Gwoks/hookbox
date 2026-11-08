//! Outgoing choke point for every mock-plane (P1) response.
//!
//! A mock response is owner-authored (rules, tunnels, MITM upstreams), and
//! on the path-fallback `/e/<token>` it is served from the APP origin — the
//! same origin as the SPA and its session cookie. Hardening:
//!   * an appended `Content-Security-Policy: sandbox …` (no
//!     `allow-same-origin`) gives any HTML/JS an opaque origin, so it can
//!     neither read app storage nor make same-origin credentialed calls;
//!   * `Clear-Site-Data` is dropped (it could wipe the owner's session);
//!   * a `Set-Cookie` for either session cookie name is dropped (session
//!     fixation / overwrite);
//!   * on the app origin every `Set-Cookie` is dropped.
//!
//! Traces are captured inside the engine before this runs, so stored trace
//! headers are unchanged.

use axum::http::header::{CONTENT_SECURITY_POLICY, SET_COOKIE};
use axum::http::{HeaderValue, Response};

use super::is_session_cookie;

/// Deliberately WITHOUT `allow-same-origin`.
pub const MOCK_SANDBOX_CSP: &str =
    "sandbox allow-scripts allow-forms allow-popups allow-modals allow-downloads";

/// Apply the mock-plane response policy. `same_origin` is true when the
/// response is served on the app origin (path-fallback `/e/<token>`), false
/// on a wildcard `<token>.<MOCK_DOMAIN>` host.
pub fn harden_mock_response<B>(resp: &mut Response<B>, same_origin: bool) {
    let headers = resp.headers_mut();
    // APPEND: a rule's own CSP still applies; browsers enforce every policy.
    headers.append(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(MOCK_SANDBOX_CSP),
    );
    headers.remove("clear-site-data");
    if headers.get(SET_COOKIE).is_none() {
        return;
    }
    let kept: Vec<HeaderValue> = if same_origin {
        Vec::new()
    } else {
        headers
            .get_all(SET_COOKIE)
            .iter()
            .filter(|v| !is_session_cookie(&String::from_utf8_lossy(v.as_bytes())))
            .cloned()
            .collect()
    };
    headers.remove(SET_COOKIE);
    for v in kept {
        headers.append(SET_COOKIE, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(pairs: &[(&str, &str)]) -> Response<()> {
        let mut r = Response::new(());
        for (k, v) in pairs {
            r.headers_mut().append(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                v.parse().unwrap(),
            );
        }
        r
    }

    fn values<'a>(r: &'a Response<()>, name: &str) -> Vec<&'a str> {
        r.headers()
            .get_all(name)
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect()
    }

    #[test]
    fn appends_sandbox_csp_and_keeps_the_rules_own() {
        let mut r = resp(&[("content-security-policy", "default-src 'none'")]);
        harden_mock_response(&mut r, false);
        assert_eq!(
            values(&r, "content-security-policy"),
            vec!["default-src 'none'", MOCK_SANDBOX_CSP]
        );
        assert!(!MOCK_SANDBOX_CSP.contains("allow-same-origin"));
    }

    #[test]
    fn strips_clear_site_data_always() {
        let mut r = resp(&[("clear-site-data", "\"cookies\"")]);
        harden_mock_response(&mut r, false);
        assert!(r.headers().get("clear-site-data").is_none());
    }

    #[test]
    fn wildcard_host_keeps_foreign_cookies_but_drops_session_ones() {
        let mut r = resp(&[
            ("set-cookie", "foo=bar"),
            ("set-cookie", "hookbox_key=evil; Path=/"),
            ("set-cookie", "__Host-hookbox_key=evil; Path=/; Secure"),
            ("set-cookie", "baz=1"),
        ]);
        harden_mock_response(&mut r, false);
        assert_eq!(values(&r, "set-cookie"), vec!["foo=bar", "baz=1"]);
    }

    #[test]
    fn app_origin_drops_every_set_cookie() {
        let mut r = resp(&[("set-cookie", "foo=bar"), ("set-cookie", "hookbox_key=x")]);
        harden_mock_response(&mut r, true);
        assert!(r.headers().get("set-cookie").is_none());
    }
}
