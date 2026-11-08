//! Forged-request protection for every cookie-authenticated (or
//! cookie-setting) management call.
//!
//! `SameSite=Strict` already keeps the cookie off cross-site requests; this
//! is the defense in depth for same-SITE attackers (a sibling subdomain such
//! as a wildcard mock host `<token>.<MOCK_DOMAIN>` serving owner-authored
//! HTML) and for browsers without SameSite:
//!   * the custom `x-hookbox: 1` header forces a CORS preflight that no
//!     foreign origin can pass (HookBox's `/api` sends no CORS grants), and
//!   * `origin_allowed` rejects any request a browser labels as coming from
//!     another origin, via `Sec-Fetch-Site` and/or `Origin`.

use axum::http::header::{HOST, ORIGIN};
use axum::http::HeaderMap;

use super::CSRF_HEADER;
use crate::config::Config;
use crate::error::ApiError;

/// Lower-cased hostname (port dropped, IPv6 kept bracketed) of an
/// `scheme://authority` origin, or `None` when it does not parse.
fn origin_hostname(origin: &str) -> Option<String> {
    let url = url::Url::parse(origin).ok()?;
    url.host_str().map(str::to_ascii_lowercase)
}

/// Hostname of the request's own `Host` header.
fn host_hostname(headers: &HeaderMap) -> Option<String> {
    let host = headers.get(HOST)?.to_str().ok()?.trim();
    if host.is_empty() {
        return None;
    }
    origin_hostname(&format!("http://{host}"))
}

/// `Sec-Fetch-Site` (sent by every modern browser) must say the request is
/// same-origin or user-initiated (`none`) when present.
fn fetch_site_allowed(headers: &HeaderMap) -> bool {
    match headers.get("sec-fetch-site") {
        None => true,
        Some(v) => v
            .to_str()
            .map(|s| {
                matches!(
                    s.trim().to_ascii_lowercase().as_str(),
                    "same-origin" | "none"
                )
            })
            .unwrap_or(false),
    }
}

/// Whether the request may act on the owner session: rejected when
/// `Sec-Fetch-Site` says cross-origin; allowed when there is no `Origin`
/// (curl, same-origin GETs); rejected for `Origin: null` or an unparsable
/// origin; otherwise the Origin hostname (port ignored) must equal the
/// `Host` hostname or the `PUBLIC_BASE_URL` hostname.
pub fn origin_allowed(headers: &HeaderMap, cfg: &Config) -> bool {
    if !fetch_site_allowed(headers) {
        return false;
    }
    let Some(origin) = headers.get(ORIGIN) else {
        return true;
    };
    let Some(origin) = origin.to_str().ok().map(str::trim) else {
        return false;
    };
    if origin.eq_ignore_ascii_case("null") {
        return false;
    }
    let Some(origin_host) = origin_hostname(origin) else {
        return false;
    };
    host_hostname(headers).as_deref() == Some(origin_host.as_str())
        || origin_hostname(&cfg.public_base_url).as_deref() == Some(origin_host.as_str())
}

/// The custom-header half: `x-hookbox: 1`.
fn has_csrf_header(headers: &HeaderMap) -> bool {
    headers
        .get(CSRF_HEADER)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim() == "1")
}

/// `x-hookbox: 1` AND `origin_allowed`, else `403 csrf_rejected`. Runs
/// before any credential lookup.
pub fn csrf_guard(headers: &HeaderMap, cfg: &Config) -> Result<(), ApiError> {
    if has_csrf_header(headers) && origin_allowed(headers, cfg) {
        Ok(())
    } else {
        Err(ApiError::csrf_rejected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{config_from, test_config};

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(HOST, "app.local:8080".parse().unwrap());
        h.insert(CSRF_HEADER, "1".parse().unwrap());
        for (k, v) in pairs {
            h.insert(
                axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(),
                v.parse().unwrap(),
            );
        }
        h
    }

    fn allowed(pairs: &[(&str, &str)]) -> bool {
        csrf_guard(&headers(pairs), &test_config()).is_ok()
    }

    #[test]
    fn no_origin_is_allowed() {
        assert!(allowed(&[]));
    }

    #[test]
    fn null_or_unparsable_origin_is_rejected() {
        assert!(!allowed(&[("origin", "null")]));
        assert!(!allowed(&[("origin", "NULL")]));
        assert!(!allowed(&[("origin", "not a url")]));
    }

    #[test]
    fn same_host_on_another_port_is_allowed() {
        assert!(allowed(&[("origin", "http://app.local:5173")]));
        assert!(allowed(&[("origin", "https://APP.local")]));
    }

    #[test]
    fn foreign_and_mock_subdomain_origins_are_rejected() {
        assert!(!allowed(&[("origin", "https://evil.example")]));
        assert!(!allowed(&[("origin", "https://tok1234567.mock.local")]));
        assert!(!allowed(&[("origin", "https://app.local.evil.example")]));
    }

    #[test]
    fn public_base_url_host_is_allowed() {
        let cfg = config_from(&[("PUBLIC_BASE_URL", "https://hookbox.example.com")]);
        let h = headers(&[
            ("host", "127.0.0.1:8080"),
            ("origin", "https://hookbox.example.com"),
        ]);
        assert!(csrf_guard(&h, &cfg).is_ok());
        let h = headers(&[
            ("host", "127.0.0.1:8080"),
            ("origin", "https://other.example"),
        ]);
        assert!(csrf_guard(&h, &cfg).is_err());
    }

    #[test]
    fn sec_fetch_site_cross_or_same_site_is_rejected() {
        assert!(!allowed(&[("sec-fetch-site", "cross-site")]));
        assert!(!allowed(&[("sec-fetch-site", "same-site")]));
        assert!(allowed(&[("sec-fetch-site", "same-origin")]));
        assert!(allowed(&[("sec-fetch-site", "none")]));
        // A same-host Origin cannot launder a cross-site fetch label.
        assert!(!allowed(&[
            ("sec-fetch-site", "same-site"),
            ("origin", "http://app.local"),
        ]));
    }

    #[test]
    fn missing_or_wrong_custom_header_is_403() {
        let mut h = headers(&[]);
        h.remove(CSRF_HEADER);
        let err = csrf_guard(&h, &test_config()).unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::FORBIDDEN);
        assert_eq!(err.code, "csrf_rejected");
        assert!(!allowed(&[(CSRF_HEADER, "0")]));
        assert!(!allowed(&[(CSRF_HEADER, "true")]));
        assert!(allowed(&[(CSRF_HEADER, " 1 ")]));
    }

    #[test]
    fn origin_allowed_ignores_the_custom_header() {
        let mut h = headers(&[]);
        h.remove(CSRF_HEADER);
        assert!(origin_allowed(&h, &test_config()));
    }
}
