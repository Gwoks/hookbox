//! `default_mode = "echo"`: the echo payload the client receives, and the
//! redacted rebuild of it that is persisted instead (AC-S3).

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::exchange::MockRequest;
use crate::net::headers::{contains_ci, redact, REDACTED, TOKEN_BEARING_REQUEST_HEADERS};

/// The echo the CLIENT receives: the raw (lower-cased) request headers
/// (AC-72, the §2 non-goal).
pub(super) fn client_echo(req: &MockRequest) -> Value {
    echo_payload(
        &req.method,
        &req.path,
        &req.query,
        &req.headers,
        &req.body_text,
    )
}

/// The echo bytes PERSISTED as the trace's `response_body`: the same payload
/// rebuilt from `redact_echo_persisted_headers`.
pub(super) fn persisted_echo(req: &MockRequest) -> Vec<u8> {
    let headers = redact_echo_persisted_headers(&req.headers, &req.token);
    let payload = echo_payload(&req.method, &req.path, &req.query, &headers, &req.body_text);
    serde_json::to_vec(&payload).unwrap_or_default()
}

/// The AC-S3 persist-path redaction (`redact()`, masking
/// authorization/cookie/x-owner-id) PLUS TWO follow-ups that must be applied
/// together (hookbox-mun.37 + hookbox-mun.38):
///   1. hookbox-mun.36: drop `TOKEN_BEARING_REQUEST_HEADERS` (`host`/
///      `origin`/`referer`) entirely — the structural token carriers in
///      wildcard mock-host mode, the same set the public share projection
///      drops from `request_headers`. That NAME-based list on its own only
///      closes the structural carriers; do NOT extend it to chase the next
///      proxy header (hookbox-mun.37) — step 2 is what generalizes the fix.
///   2. hookbox-mun.37: mask any SURVIVING header whose VALUE contains
///      `token` to `REDACTED`, mirroring
///      `routes::share::projection::mask_token_in_value` — a reverse proxy's
///      `X-Forwarded-Host`, `X-Forwarded-Server`, `Forwarded`,
///      `X-Original-URI`, `X-Forwarded-Uri` or `X-Envoy-Original-Path` (all
///      added by default by Apache/Caddy/Traefik/ingress-nginx/Envoy) is not
///      in the fixed 3-name drop list above, so without this the endpoint
///      token re-enters an anonymous share viewer's response through the
///      echo BODY instead of the header maps — the same class as channel
///      A/B/C, closed generically instead of by enumerating one more name.
///
/// Both this function and `mask_token_in_value` share `net::headers::contains_ci`,
/// which is case-insensitive (hookbox-mun.38: nginx's `$host` variable is
/// documented as lowercase, so a case-sensitive compare lets a case-folded
/// token through) — so the two copies of this check cannot silently drift
/// apart again.
///
/// Applied ONLY to the `headers` sub-object rebuilt for the persisted
/// `response_body` of a `default_mode = "echo"` row (`persisted_echo`) — the
/// client's own echo body still carries the raw headers (AC-72, the §2
/// non-goal), and every other row shape / column (including the stored
/// `request_headers` column, the owner Inspector and F5's CSV) is untouched.
pub(super) fn redact_echo_persisted_headers(
    headers: &BTreeMap<String, String>,
    token: &str,
) -> BTreeMap<String, String> {
    redact(headers)
        .into_iter()
        .filter(|(k, _)| !TOKEN_BEARING_REQUEST_HEADERS.contains(&k.to_ascii_lowercase().as_str()))
        .map(|(k, v)| {
            if contains_ci(&v, token) {
                (k, REDACTED.to_string())
            } else {
                (k, v)
            }
        })
        .collect()
}

/// The `default_mode = "echo"` payload shape, shared so the client build
/// (unredacted `headers`) and the AC-S3 persisted rebuild (redacted
/// `headers`) can never drift apart.
fn echo_payload(
    method: &str,
    path: &str,
    query: &BTreeMap<String, String>,
    headers: &BTreeMap<String, String>,
    body: &str,
) -> Value {
    json!({ "method": method, "path": path, "query": query, "headers": headers, "body": body })
}

#[cfg(test)]
mod tests {
    use super::*;

    // hookbox-mun.37 + hookbox-mun.38: a proxy header whose NAME is not in
    // `TOKEN_BEARING_REQUEST_HEADERS` but whose VALUE carries the endpoint token
    // (in ANY letter case) must be masked, not just dropped-by-name headers.
    #[test]
    fn redact_echo_persisted_headers_masks_any_value_containing_the_token() {
        let token = "oPp8tASu3i";
        let mut headers = BTreeMap::new();
        headers.insert("host".to_string(), format!("{token}.mock.local"));
        headers.insert(
            "x-forwarded-host".to_string(),
            format!("{token}.mock.local"),
        );
        headers.insert(
            "x-forwarded-server".to_string(),
            format!("{token}.mock.local"),
        );
        headers.insert(
            "forwarded".to_string(),
            format!("for=203.0.113.9;host={token}.mock.local;proto=https"),
        );
        headers.insert(
            "x-envoy-original-path".to_string(),
            format!("/e/{token}/ingress"),
        );
        headers.insert("accept".to_string(), "application/json".to_string());
        let out = redact_echo_persisted_headers(&headers, token);

        // Structural name-drop headers are absent entirely.
        assert!(!out.contains_key("host"));
        // Every OTHER header whose value carries the token is masked, not
        // just the 3-name list — this is the generic fix, not a longer
        // enumeration.
        assert_eq!(out["x-forwarded-host"], "<redacted>");
        assert_eq!(out["x-forwarded-server"], "<redacted>");
        assert_eq!(out["forwarded"], "<redacted>");
        assert_eq!(out["x-envoy-original-path"], "<redacted>");
        // A header carrying no token value is untouched.
        assert_eq!(out["accept"], "application/json");
    }

    #[test]
    fn redact_echo_persisted_headers_masks_a_case_folded_token() {
        // hookbox-mun.38: nginx's `$host` variable is documented as
        // lowercase, so the value arrives case-folded relative to the
        // mixed-case token.
        let token = "ixaU3viom4";
        let mut headers = BTreeMap::new();
        headers.insert(
            "x-forwarded-host".to_string(),
            "ixau3viom4.mock.local".to_string(),
        );
        let out = redact_echo_persisted_headers(&headers, token);
        assert_eq!(out["x-forwarded-host"], "<redacted>");
    }

    #[test]
    fn echo_payload_shape_is_stable() {
        let mut query = BTreeMap::new();
        query.insert("q".to_string(), "1".to_string());
        let mut headers = BTreeMap::new();
        headers.insert("authorization".to_string(), "Bearer secret".to_string());
        let v = echo_payload("GET", "/p", &query, &headers, "body-text");
        assert_eq!(v["method"], json!("GET"));
        assert_eq!(v["path"], json!("/p"));
        assert_eq!(v["query"]["q"], json!("1"));
        assert_eq!(v["headers"]["authorization"], json!("Bearer secret"));
        assert_eq!(v["body"], json!("body-text"));
    }
}
