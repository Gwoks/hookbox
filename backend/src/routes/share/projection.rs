//! The PUBLIC trace projection (§5.11, AC-34/102, AC-S1/S2): standalone
//! field-by-field structs plus the header filters that drop/mask anything that
//! identifies the endpoint or carries a credential. Applied ONLY to the public
//! share viewer — storage, the owner Inspector and F5's CSV stay verbatim.

use serde_json::{json, Value};
use sqlx::{sqlite::SqliteRow, Row};

use crate::helpers::contains_ci;
use crate::models::{PublicRequestDetail, PublicRequestSummary};
use crate::util::json::parse_json_or;
use crate::util::time::to_rfc3339;

/// Dropped entirely from the PUBLIC projection: internal markers that identify
/// the endpoint or the matched rule. Prefix rule, not an enumeration, so a
/// future x-hookbox-* header cannot leak by omission. (AC-S1)
const PUBLIC_RESPONSE_HEADER_DROP_PREFIX: &str = "x-hookbox-";

/// Value replaced with "<redacted>" in the PUBLIC projection: credential-bearing
/// response headers. Key is KEPT so the viewer can see one was sent. (AC-S1)
const PUBLIC_RESPONSE_HEADER_REDACT: [&str; 5] = [
    "set-cookie",
    "set-cookie2",
    "authorization",
    "proxy-authenticate",
    "www-authenticate",
];

/// Dropped entirely from the PUBLIC projection's `request_headers` (AC-43,
/// AC-S2, hookbox-mun.34): in wildcard mock-host mode the `Host` header IS
/// `<token>.<MOCK_DOMAIN>` on EVERY row, and `Origin`/`Referer` echo it back
/// on a cross-origin browser request. Unlike `REDACT_HEADERS`
/// (`helpers.rs`) — a capability scrubber applied at persist time — this is
/// a token-disclosure scrubber applied ONLY at the public projection;
/// storage and the owner Inspector keep these headers verbatim (§5.11).
const PUBLIC_REQUEST_HEADER_DROP: [&str; 3] = ["host", "origin", "referer"];

/// Mask `value` to `"<redacted>"` iff it is a JSON string containing `token`
/// case-insensitively — the second half of the AC-S2/AC-43 fix, applied to
/// BOTH `request_headers` and `response_headers` in the PUBLIC projection so
/// a server-generated echo of the token (e.g. a CORS
/// `access-control-allow-origin` reflecting the wildcard mock `Origin`, or a
/// reverse proxy's `X-Forwarded-Host`) cannot survive the name-based filters
/// above. Never applied to `path`, `query_params`, or either body — those
/// are caller-supplied and a caller choosing to paste the token into their
/// own body is out of scope (AC-S2). The comparison is case-insensitive
/// (`helpers::contains_ci`, hookbox-mun.38): nginx's `$host` variable is
/// documented as lowercase, so a case-sensitive `contains` let a
/// case-folded token survive verbatim. The `response_body` column is NOT
/// filtered here at all (§5.11) — that channel is closed on the persist path
/// instead, in `interceptor::engine::redact_echo_persisted_headers`, which
/// shares this exact case-insensitive comparison via `helpers::contains_ci`
/// so the two can never drift apart again (hookbox-mun.37).
fn mask_token_in_value(token: &str, value: Value) -> Value {
    match &value {
        Value::String(s) if contains_ci(s, token) => json!("<redacted>"),
        _ => value,
    }
}

/// Filter `response_headers` for the PUBLIC projection ONLY (§5.11, AC-S1,
/// AC-S2): drop every `x-hookbox-*` key entirely, mask the five
/// credential-bearing keys to `<redacted>` (key kept, value replaced), then
/// mask any surviving value that contains `token` — closes the CORS-echo
/// channel where `access-control-allow-origin` reflects a wildcard `Origin:
/// https://<token>.<MOCK_DOMAIN>` sent by the caller (hookbox-mun.34).
/// Storage, the owner Inspector and F5's CSV all stay verbatim — this filter
/// is applied nowhere else in the codebase.
fn filter_public_response_headers(raw: Value, token: &str) -> Value {
    let map = match raw {
        Value::Object(m) => m,
        _ => return json!({}),
    };
    let mut out = serde_json::Map::new();
    for (k, v) in map {
        let kl = k.to_ascii_lowercase();
        if kl.starts_with(PUBLIC_RESPONSE_HEADER_DROP_PREFIX) {
            continue;
        }
        if PUBLIC_RESPONSE_HEADER_REDACT.contains(&kl.as_str()) {
            out.insert(k, json!("<redacted>"));
        } else {
            out.insert(k, mask_token_in_value(token, v));
        }
    }
    Value::Object(out)
}

/// Filter `request_headers` for the PUBLIC projection ONLY (AC-43, AC-S2,
/// hookbox-mun.34): drop `host`/`origin`/`referer` entirely — structural
/// carriers of the token in wildcard mock-host mode — then mask any
/// surviving value that contains `token`. Storage and the owner Inspector
/// keep `request_headers` verbatim (§5.11); this filter is applied nowhere
/// else in the codebase.
fn filter_public_request_headers(raw: Value, token: &str) -> Value {
    let map = match raw {
        Value::Object(m) => m,
        _ => return json!({}),
    };
    let mut out = serde_json::Map::new();
    for (k, v) in map {
        let kl = k.to_ascii_lowercase();
        if PUBLIC_REQUEST_HEADER_DROP.contains(&kl.as_str()) {
            continue;
        }
        out.insert(k, mask_token_in_value(token, v));
    }
    Value::Object(out)
}

pub(super) fn public_request_summary(row: &SqliteRow) -> PublicRequestSummary {
    PublicRequestSummary {
        id: row.get("id"),
        method: row.get("method"),
        path: row.get("path"),
        status_code: row.get("status_code"),
        served_by: row.get("served_by"),
        duration_ms: row.get("duration_ms"),
        timestamp: to_rfc3339(row.get("created_at")).unwrap_or_default(),
    }
}

/// `token` is the resolved share row's OWN endpoint token (never taken from
/// the caller) — used only to mask its own reappearance in the projection
/// (AC-43, AC-S2).
pub(super) fn public_request_detail(row: &SqliteRow, token: &str) -> PublicRequestDetail {
    PublicRequestDetail {
        id: row.get("id"),
        method: row.get("method"),
        path: row.get("path"),
        status_code: row.get("status_code"),
        served_by: row.get("served_by"),
        duration_ms: row.get("duration_ms"),
        timestamp: to_rfc3339(row.get("created_at")).unwrap_or_default(),
        request_headers: filter_public_request_headers(
            parse_json_or(row.get("request_headers"), json!({})),
            token,
        ),
        query_params: parse_json_or(row.get("query_params"), json!({})),
        request_body: row.get("request_body"),
        response_headers: filter_public_response_headers(
            parse_json_or(row.get("response_headers"), json!({})),
            token,
        ),
        response_body: row.get("response_body"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_drops_x_hookbox_prefix_and_redacts_credential_headers() {
        let raw = json!({
            "x-hookbox-endpoint": "tok1234567",
            "x-hookbox-rule-id": "1",
            "set-cookie": "sid=abc",
            "content-type": "application/json",
        });
        let out = filter_public_response_headers(raw, "tok1234567");
        let obj = out.as_object().unwrap();
        assert!(!obj.contains_key("x-hookbox-endpoint"));
        assert!(!obj.contains_key("x-hookbox-rule-id"));
        assert_eq!(obj["set-cookie"], json!("<redacted>"));
        assert_eq!(obj["content-type"], json!("application/json"));
    }

    #[test]
    fn filter_response_headers_masks_any_value_containing_the_token() {
        // hookbox-mun.34 channel A: a CORS echo of the wildcard mock Origin
        // survives the name-based filter and must still be masked.
        let raw = json!({
            "access-control-allow-origin": "https://sx37Uac9ty.mock.local",
            "content-type": "application/json",
        });
        let out = filter_public_response_headers(raw, "sx37Uac9ty");
        let obj = out.as_object().unwrap();
        assert_eq!(obj["access-control-allow-origin"], json!("<redacted>"));
        assert_eq!(obj["content-type"], json!("application/json"));
    }

    #[test]
    fn filter_request_headers_drops_host_origin_referer_and_masks_token_value() {
        // hookbox-mun.34 channel B: Host structurally embeds the token in
        // wildcard mode; Origin/Referer can echo it too.
        let raw = json!({
            "host": "Q3L3jRQ7oY.mock.local",
            "origin": "https://Q3L3jRQ7oY.mock.local",
            "referer": "https://Q3L3jRQ7oY.mock.local/x",
            "x-forwarded-for": "https://Q3L3jRQ7oY.mock.local/smuggled",
            "accept": "application/json",
        });
        let out = filter_public_request_headers(raw, "Q3L3jRQ7oY");
        let obj = out.as_object().unwrap();
        assert!(!obj.contains_key("host"));
        assert!(!obj.contains_key("origin"));
        assert!(!obj.contains_key("referer"));
        assert_eq!(obj["x-forwarded-for"], json!("<redacted>"));
        assert_eq!(obj["accept"], json!("application/json"));
    }

    #[test]
    fn filter_masks_a_case_folded_token_case_insensitively() {
        // hookbox-mun.38: nginx's `$host` variable is documented as
        // lowercase, so `proxy_set_header X-Forwarded-Host $host;` delivers
        // a case-FOLDED token. A case-sensitive `contains` used to let this
        // survive verbatim in both request AND response header filters.
        let token = "ixaU3viom4"; // mixed-case, as gen_token produces it
        let req_raw = json!({
            "x-forwarded-host": "ixau3viom4.mock.local", // lowercased
            "accept": "application/json",
        });
        let req_out = filter_public_request_headers(req_raw, token);
        assert_eq!(
            req_out["x-forwarded-host"],
            json!("<redacted>"),
            "a case-folded token in request_headers must still be masked"
        );
        assert_eq!(req_out["accept"], json!("application/json"));

        let resp_raw = json!({
            "access-control-allow-origin": format!("https://{}.mock.local", token.to_ascii_uppercase()),
            "content-type": "application/json",
        });
        let resp_out = filter_public_response_headers(resp_raw, token);
        assert_eq!(
            resp_out["access-control-allow-origin"],
            json!("<redacted>"),
            "an upper-cased token in response_headers must still be masked"
        );
        assert_eq!(resp_out["content-type"], json!("application/json"));
    }
}
