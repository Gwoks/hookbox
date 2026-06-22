//! Auto-CORS engine — PORT of `app/interceptor/cors.py` (§5.5, AC-33..36, AC-S11/S12).
//!
//! P1-only wide-open dynamic CORS with zero user configuration:
//!   * Preflight `OPTIONS` (when `cors_enabled`) → `204` with reflected `Origin`,
//!     reflected `Access-Control-Request-Headers` (else `*`), `Allow-Methods`,
//!     `Max-Age: 600`, `Vary: Origin`.
//!   * Every non-preflight P1 response carries reflected `Allow-Origin` (or `*`),
//!     `Expose-Headers: *`, `Vary: Origin`.
//!
//! Two security invariants (AC-S11/S12): `Access-Control-Allow-Credentials` is
//! NEVER emitted (we reflect the Origin rather than pairing `*` + credentials);
//! and these headers are produced ONLY here, on the P1 plane — the management API
//! (P2) emits no wildcard CORS because nothing on that plane calls this module.
//! When `cors_enabled=false` the per-response set is empty but `OPTIONS` is still
//! a deterministic `204` (never falls through to a rule/404).

use std::collections::BTreeMap;

/// Methods always advertised as allowed on the mock surface (§5.5, frozen).
pub const ALLOW_METHODS: &str = "GET,POST,PUT,PATCH,DELETE,OPTIONS,HEAD";
/// Preflight cache lifetime in seconds (§5.5).
pub const MAX_AGE: &str = "600";

/// Reflect the request `Origin` (usable by a credential-less browser fetch),
/// falling back to `*` when absent — never `*` + credentials (AC-S12).
fn reflected_origin(req_headers: &BTreeMap<String, String>) -> String {
    req_headers
        .get("origin")
        .cloned()
        .unwrap_or_else(|| "*".to_string())
}

/// The per-response CORS header set for a non-preflight P1 response (AC-35).
/// Empty when CORS is disabled. Never includes `Allow-Credentials`.
pub fn response_headers(
    req_headers: &BTreeMap<String, String>,
    cors_enabled: bool,
) -> Vec<(String, String)> {
    if !cors_enabled {
        return Vec::new();
    }
    vec![
        (
            "Access-Control-Allow-Origin".into(),
            reflected_origin(req_headers),
        ),
        ("Access-Control-Expose-Headers".into(), "*".into()),
        ("Vary".into(), "Origin".into()),
    ]
}

/// The preflight (`OPTIONS`) header set (AC-34). Reflects the requested
/// `Access-Control-Request-Headers` (or `*`). Empty when CORS disabled.
pub fn preflight_headers(
    req_headers: &BTreeMap<String, String>,
    cors_enabled: bool,
) -> Vec<(String, String)> {
    if !cors_enabled {
        return Vec::new();
    }
    let allow_headers = req_headers
        .get("access-control-request-headers")
        .cloned()
        .unwrap_or_else(|| "*".to_string());
    vec![
        (
            "Access-Control-Allow-Origin".into(),
            reflected_origin(req_headers),
        ),
        ("Access-Control-Allow-Methods".into(), ALLOW_METHODS.into()),
        ("Access-Control-Allow-Headers".into(), allow_headers),
        ("Access-Control-Max-Age".into(), MAX_AGE.into()),
        ("Vary".into(), "Origin".into()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn never_emits_allow_credentials() {
        let h = headers(&[("origin", "https://app.example.com")]);
        for set in [response_headers(&h, true), preflight_headers(&h, true)] {
            assert!(!set
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("access-control-allow-credentials")));
        }
    }

    #[test]
    fn reflects_origin_and_request_headers() {
        let h = headers(&[
            ("origin", "https://x.io"),
            ("access-control-request-headers", "x-custom"),
        ]);
        let pf = preflight_headers(&h, true);
        assert!(pf
            .iter()
            .any(|(k, v)| k == "Access-Control-Allow-Origin" && v == "https://x.io"));
        assert!(pf
            .iter()
            .any(|(k, v)| k == "Access-Control-Allow-Headers" && v == "x-custom"));
        assert!(pf
            .iter()
            .any(|(k, v)| k == "Access-Control-Allow-Methods" && v == ALLOW_METHODS));
    }

    #[test]
    fn falls_back_to_star_without_origin() {
        let h = headers(&[]);
        let rh = response_headers(&h, true);
        assert!(rh
            .iter()
            .any(|(k, v)| k == "Access-Control-Allow-Origin" && v == "*"));
    }

    #[test]
    fn disabled_yields_empty_sets() {
        let h = headers(&[("origin", "https://x.io")]);
        assert!(response_headers(&h, false).is_empty());
        assert!(preflight_headers(&h, false).is_empty());
    }
}
