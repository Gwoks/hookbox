//! Named header policies and header plumbing shared by every plane.
//!
//! Each strip/drop/redact rule is a named constant or predicate here, so the
//! mock plane, MITM proxy, tunnel replay, trace persistence and the public
//! share projection all reference one definition. The three hop-by-hop
//! policies below are DIFFERENT SETS ON PURPOSE (each pinned by a test); they
//! are composed from the shared constants rather than merged.

use std::collections::BTreeMap;

use axum::http::{HeaderMap, HeaderName, HeaderValue};

/// The placeholder written in place of a masked header value.
pub(crate) const REDACTED: &str = "<redacted>";

/// Request headers that structurally carry the endpoint token in wildcard
/// mock-host mode: `Host` IS `<token>.<MOCK_DOMAIN>` on every request, and a
/// cross-origin browser echoes it back via `Origin`/`Referer` (AC-43, AC-S2,
/// hookbox-mun.34/36). Dropped entirely from the public share projection's
/// `request_headers` and from a persisted echo body's `headers` sub-object.
pub(crate) const TOKEN_BEARING_REQUEST_HEADERS: [&str; 3] = ["host", "origin", "referer"];

/// RFC 7230 §6.1 hop-by-hop headers.
const RFC7230_HOP_BY_HOP: [&str; 8] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

/// Body-framing headers: dropped wherever we re-send a body ourselves.
const BODY_FRAMING: [&str; 2] = ["content-length", "content-encoding"];

/// Upstream CORS headers: P1 injects its own auto-CORS instead.
const UPSTREAM_CORS: [&str; 5] = [
    "access-control-allow-origin",
    "access-control-allow-credentials",
    "access-control-expose-headers",
    "access-control-max-age",
    "vary",
];

/// The hop-by-hop subset the tunnel replay strips from a CLI reply. Narrower
/// than RFC 7230 on purpose: aligning it is a tracked follow-up (B9), not a
/// silent refactor side effect.
const TUNNEL_HOP_BY_HOP: [&str; 3] = ["connection", "keep-alive", "transfer-encoding"];

/// Sensitive inbound headers that must never be forwarded upstream (AC-S9) on
/// top of the hop-by-hop set. The owner capability rides `Authorization` to
/// the management plane only; it is dropped here so it never reaches an MITM
/// target.
const SENSITIVE_FORWARD_HEADERS: [&str; 5] = [
    "authorization",
    "cookie",
    "x-owner-id",
    "x-user-id",
    "x-hookbox-cap",
];

/// Owner-capability / cookie headers redacted before a trace is persisted OR
/// published to the feed (AC-S15 / engine `_redact`). Case-insensitive names.
const REDACT_HEADERS: [&str; 3] = ["authorization", "cookie", "x-owner-id"];

fn in_set(set: &[&str], name: &str) -> bool {
    set.iter().any(|h| h.eq_ignore_ascii_case(name))
}

/// Inbound request headers never forwarded to an MITM upstream: RFC 7230
/// hop-by-hop + `host` + the sensitive set (AC-S9).
pub(crate) fn is_forward_stripped(name: &str) -> bool {
    in_set(&RFC7230_HOP_BY_HOP, name)
        || name.eq_ignore_ascii_case("host")
        || in_set(&SENSITIVE_FORWARD_HEADERS, name)
}

/// Upstream MITM response headers never copied back: RFC 7230 hop-by-hop +
/// body framing + `set-cookie` + upstream CORS (we inject our own P1 CORS and
/// re-send the body).
pub(crate) fn is_mitm_response_stripped(name: &str) -> bool {
    in_set(&RFC7230_HOP_BY_HOP, name)
        || in_set(&BODY_FRAMING, name)
        || name.eq_ignore_ascii_case("set-cookie")
        || in_set(&UPSTREAM_CORS, name)
}

/// Tunnel CLI reply headers never copied onto the mock response: the tunnel
/// hop-by-hop subset + body framing.
pub(crate) fn is_tunnel_response_stripped(name: &str) -> bool {
    in_set(&TUNNEL_HOP_BY_HOP, name) || in_set(&BODY_FRAMING, name)
}

/// Redact sensitive headers (lower-cased names) before persisting/publishing a
/// trace. PORT of `engine.py::_redact`.
pub(crate) fn redact(headers: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    headers
        .iter()
        .map(|(k, v)| {
            if in_set(&REDACT_HEADERS, k) {
                (k.clone(), REDACTED.to_string())
            } else {
                (k.clone(), v.clone())
            }
        })
        .collect()
}

/// Headers safe to forward to an MITM upstream (AC-S9). PORT of
/// `helpers.py::strip_forward_headers`.
pub(crate) fn strip_forward_headers(
    headers: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    headers
        .iter()
        .filter(|(k, _)| !is_forward_stripped(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Case-insensitive substring test — does `haystack` contain `needle`
/// anywhere, ignoring ASCII letter case? An empty `needle` (a blank or
/// unresolved token) never matches, so it can never "match" every value.
///
/// Shared by the two independent token-disclosure masks that must never
/// drift apart again (`routes::share::projection::mask_token_in_value` and
/// `interceptor::engine::echo::redact_echo_persisted_headers`, hookbox-mun.37 /
/// hookbox-mun.38): a case-SENSITIVE `str::contains` lets a case-folded
/// token survive both filters, because nginx's `$host` variable is
/// documented as returning its value in lowercase, so the near-universal
/// `proxy_set_header X-Forwarded-Host $host;` snippet delivers a lowercased
/// endpoint token to the backend even though the token alphabet itself is
/// mixed-case (`ids::gen_token`).
pub(crate) fn contains_ci(haystack: &str, needle: &str) -> bool {
    !needle.is_empty()
        && haystack
            .to_ascii_lowercase()
            .contains(&needle.to_ascii_lowercase())
}

/// Build a typed header pair, or `None` when either part is invalid (callers
/// silently skip an invalid pair — never an error path).
pub(crate) fn try_header(name: &str, value: &str) -> Option<(HeaderName, HeaderValue)> {
    let name = HeaderName::from_bytes(name.as_bytes()).ok()?;
    let value = HeaderValue::from_str(value).ok()?;
    Some((name, value))
}

/// Mark any response (success or error) `Cache-Control: no-store`.
pub(crate) fn no_store(mut resp: axum::response::Response) -> axum::response::Response {
    resp.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    resp
}

/// Every header as a `(name, value)` string pair, skipping values that are not
/// visible ASCII. Duplicate names yield one pair per value, in map order.
pub(crate) fn header_pairs(headers: &HeaderMap) -> impl Iterator<Item = (&str, &str)> {
    headers
        .iter()
        .filter_map(|(name, value)| value.to_str().ok().map(|v| (name.as_str(), v)))
}

/// Response headers as persisted on a trace (F7, R11 seam). Today an identity
/// projection.
///
/// THIS IS THE SLOT-IN POINT for the F4 security lane's public-projection
/// filter (§5.11): that filter lives in `routes/share/projection.rs` and is
/// applied ONLY to the public projection, never here — storage, the owner
/// Inspector and F5's CSV all stay verbatim (AC-S1's S-4 ruling). Note MITM
/// already strips upstream `Set-Cookie` before the response is built
/// (`interceptor/proxy.rs`), so the exposure surface here is tunnel replies
/// and rule-authored response headers.
pub(crate) fn response_headers_for_trace(headers: &HeaderMap) -> BTreeMap<String, String> {
    header_pairs(headers)
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every name any policy mentions, plus a few that no policy touches.
    const CANDIDATES: [&str; 26] = [
        "connection",
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "te",
        "trailers",
        "transfer-encoding",
        "upgrade",
        "host",
        "content-length",
        "content-encoding",
        "set-cookie",
        "access-control-allow-origin",
        "access-control-allow-credentials",
        "access-control-expose-headers",
        "access-control-max-age",
        "vary",
        "authorization",
        "cookie",
        "x-owner-id",
        "x-user-id",
        "x-hookbox-cap",
        "origin",
        "referer",
        "content-type",
        "x-custom",
    ];

    fn members(policy: fn(&str) -> bool) -> Vec<&'static str> {
        let mut out: Vec<&str> = CANDIDATES.iter().copied().filter(|n| policy(n)).collect();
        out.sort_unstable();
        out
    }

    fn sorted(mut names: Vec<&'static str>) -> Vec<&'static str> {
        names.sort_unstable();
        names
    }

    #[test]
    fn forward_strip_set_is_exact() {
        let expected = sorted(vec![
            "connection",
            "keep-alive",
            "proxy-authenticate",
            "proxy-authorization",
            "te",
            "trailers",
            "transfer-encoding",
            "upgrade",
            "host",
            "authorization",
            "cookie",
            "x-owner-id",
            "x-user-id",
            "x-hookbox-cap",
        ]);
        assert_eq!(members(is_forward_stripped), expected);
    }

    #[test]
    fn mitm_response_strip_set_is_exact() {
        let expected = sorted(vec![
            "connection",
            "keep-alive",
            "proxy-authenticate",
            "proxy-authorization",
            "te",
            "trailers",
            "transfer-encoding",
            "upgrade",
            "content-length",
            "content-encoding",
            "set-cookie",
            "access-control-allow-origin",
            "access-control-allow-credentials",
            "access-control-expose-headers",
            "access-control-max-age",
            "vary",
        ]);
        assert_eq!(members(is_mitm_response_stripped), expected);
    }

    #[test]
    fn tunnel_response_strip_set_is_exact() {
        let expected = sorted(vec![
            "connection",
            "keep-alive",
            "transfer-encoding",
            "content-length",
            "content-encoding",
        ]);
        assert_eq!(members(is_tunnel_response_stripped), expected);
    }

    #[test]
    fn policies_match_names_case_insensitively() {
        assert!(is_forward_stripped("Authorization"));
        assert!(is_mitm_response_stripped("Set-Cookie"));
        assert!(is_tunnel_response_stripped("Content-Length"));
        assert!(!is_tunnel_response_stripped("Upgrade"));
    }

    #[test]
    fn redact_masks_sensitive_headers() {
        let mut h = BTreeMap::new();
        h.insert("authorization".to_string(), "Bearer secret".to_string());
        h.insert("cookie".to_string(), "sid=1".to_string());
        h.insert("x-owner-id".to_string(), "abc".to_string());
        h.insert("accept".to_string(), "application/json".to_string());
        let out = redact(&h);
        assert_eq!(out["authorization"], "<redacted>");
        assert_eq!(out["cookie"], "<redacted>");
        assert_eq!(out["x-owner-id"], "<redacted>");
        assert_eq!(out["accept"], "application/json");
    }

    #[test]
    fn strip_forward_drops_sensitive_and_hop_by_hop() {
        let mut h = BTreeMap::new();
        h.insert("authorization".to_string(), "Bearer x".to_string());
        h.insert("host".to_string(), "victim".to_string());
        h.insert("x-hookbox-cap".to_string(), "cap".to_string());
        h.insert("accept".to_string(), "*/*".to_string());
        let out = strip_forward_headers(&h);
        assert!(!out.contains_key("authorization"));
        assert!(!out.contains_key("host"));
        assert!(!out.contains_key("x-hookbox-cap"));
        assert_eq!(out["accept"], "*/*");
    }

    #[test]
    fn contains_ci_matches_regardless_of_letter_case() {
        // The exact repro shape from hookbox-mun.38: nginx's lowercase
        // `$host` case-folds a mixed-case token.
        assert!(contains_ci("ixau3viom4.mock.local", "ixaU3viom4"));
        assert!(contains_ci("IXAU3VIOM4.MOCK.LOCAL", "ixaU3viom4"));
        assert!(contains_ci("https://Q3L3jRQ7oY.mock.local", "q3l3jrq7oy"));
        assert!(!contains_ci("no token here", "ixaU3viom4"));
        // An empty needle never matches, so a blank/unresolved token can
        // never redact every value.
        assert!(!contains_ci("anything at all", ""));
    }

    #[test]
    fn try_header_rejects_invalid_parts() {
        assert!(try_header("x-ok", "value").is_some());
        assert!(try_header("bad name", "value").is_none());
        assert!(try_header("x-ok", "bad\nvalue").is_none());
    }

    #[test]
    fn header_pairs_skips_non_ascii_values_and_keeps_duplicates() {
        let mut h = HeaderMap::new();
        h.append("set-cookie", "a=1".parse().unwrap());
        h.append("set-cookie", "b=2".parse().unwrap());
        h.insert("x-bin", HeaderValue::from_bytes(&[0xFF]).unwrap());
        let pairs: Vec<(&str, &str)> = header_pairs(&h).collect();
        assert_eq!(pairs, vec![("set-cookie", "a=1"), ("set-cookie", "b=2")]);
    }

    #[test]
    fn response_headers_for_trace_is_an_identity_projection() {
        let mut h = HeaderMap::new();
        h.insert("content-type", "application/json".parse().unwrap());
        h.insert("x-hookbox-endpoint", "tok1234567".parse().unwrap());
        let out = response_headers_for_trace(&h);
        assert_eq!(out["content-type"], "application/json");
        assert_eq!(out["x-hookbox-endpoint"], "tok1234567");
        assert_eq!(out.len(), 2);
    }
}
