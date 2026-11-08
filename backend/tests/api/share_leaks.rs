//! F4 public projection invariants: exact key allowlist, header filtering, and
//! that the endpoint token / masked values never leak through any channel
//! (AC-S1/S2/S4, AC-43, hookbox-mun.34–38).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::common::{
    add_rule, call, latest_request_detail, list_requests, mint_share, new_endpoint, patch_endpoint,
    public_detail, public_list, TestApp,
};

#[tokio::test]
async fn share_public_detail_key_allowlist_and_no_token_substring() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    add_rule(
        &app,
        &secret,
        &token,
        json!({"match":{"method":"GET","path":"/p"},"response":{"status_code":200}}),
    )
    .await;
    let host = format!("{token}.mock.local");
    // hookbox-mun.34 channel A: send an `Origin` header equal to the
    // caller's own wildcard mock host, so the Auto-CORS echo populates
    // `access-control-allow-origin` with a value containing the token —
    // exactly what a scanning viewer would do to recover it from
    // `response_headers` (AC-S2).
    let req = Request::builder()
        .method("GET")
        .uri("/p")
        .header("host", &host)
        .header("origin", format!("https://{host}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("access-control-allow-origin").unwrap(),
        format!("https://{host}").as_str(),
        "sanity: the mock response really does echo the token-bearing Origin"
    );
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let list = list_requests(&app, &secret, &token).await;
    let request_id = list.as_array().unwrap()[0]["id"].as_i64().unwrap();

    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    let (s, detail, _) = public_detail(&app, &code, request_id).await;
    assert_eq!(s, StatusCode::OK);
    let keys: std::collections::BTreeSet<&str> = detail
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    let expected: std::collections::BTreeSet<&str> = [
        "id",
        "method",
        "path",
        "status_code",
        "served_by",
        "duration_ms",
        "timestamp",
        "request_headers",
        "query_params",
        "request_body",
        "response_headers",
        "response_body",
    ]
    .into_iter()
    .collect();
    assert_eq!(keys, expected, "AC-102 public detail key set must be exact");

    // AC-S2/AC-43 (hookbox-mun.34): token absence is asserted AFTER the
    // filter, across every field that can structurally or incidentally carry
    // it — `response_headers` (the CORS echo, channel A) AND
    // `request_headers` (the wildcard `Host`/`Origin`/`Referer`, channel B).
    // `path`/`query_params`/`request_body` stay out of scope on purpose: a
    // caller choosing to paste the token into their OWN body is not a
    // HookBox-side disclosure, and an unscoped "not a substring anywhere"
    // assertion over caller-supplied fields is the flaky test the PRD
    // explicitly rejects.
    let response_headers_raw = serde_json::to_string(&detail["response_headers"]).unwrap();
    assert!(
        !response_headers_raw.contains(&token),
        "the filtered response_headers must never contain the endpoint token, got: {response_headers_raw}"
    );
    let request_headers = detail["request_headers"].as_object().unwrap();
    assert!(
        !request_headers.contains_key("host"),
        "the wildcard Host header structurally carries the token and must be dropped"
    );
    assert!(!request_headers.contains_key("origin"));
    assert!(!request_headers.contains_key("referer"));
    let request_headers_raw = serde_json::to_string(&detail["request_headers"]).unwrap();
    assert!(
        !request_headers_raw.contains(&token),
        "the filtered request_headers must never contain the endpoint token, got: {request_headers_raw}"
    );
    let identity_raw = serde_json::json!({
        "id": detail["id"], "method": detail["method"], "path": detail["path"],
        "status_code": detail["status_code"], "served_by": detail["served_by"],
        "duration_ms": detail["duration_ms"], "timestamp": detail["timestamp"],
    })
    .to_string();
    assert!(!identity_raw.contains(&token));
    // The five omitted structural fields never appear as JSON keys.
    let raw = serde_json::to_string(&detail).unwrap();
    assert!(!raw.contains("\"matched_rule_id\""));
    assert!(!raw.contains("\"overhead_ms\""));
    assert!(!raw.contains("\"state_snapshot\""));
    assert!(!raw.contains("\"trace\""));
}

#[tokio::test]
async fn share_response_headers_filtered_publicly_but_verbatim_for_owner() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    add_rule(&app, &secret, &token, json!({
        "match": {"method":"GET","path":"/p"},
        "response": {"status_code": 200, "headers": {"set-cookie": "sid=abc"}, "content_type":"text/plain","body_template":"ok"}
    })).await;
    let host = format!("{token}.mock.local");
    call(&app, "GET", &host, "/p", None, None).await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let list = list_requests(&app, &secret, &token).await;
    let request_id = list.as_array().unwrap()[0]["id"].as_i64().unwrap();

    // Owner Inspector: verbatim (S-4 ruling) — including HookBox's own
    // x-hookbox-* identifying headers.
    let (s, owner_detail, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/requests/{request_id}"),
        Some(&secret),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        owner_detail["response_headers"]["set-cookie"],
        json!("sid=abc")
    );
    assert!(owner_detail["response_headers"]
        .as_object()
        .unwrap()
        .keys()
        .any(|k| k.to_ascii_lowercase().starts_with("x-hookbox-")));

    // Public detail: masked + every x-hookbox-* key dropped entirely (AC-S1).
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();
    let (s, pub_detail, _) = public_detail(&app, &code, request_id).await;
    assert_eq!(s, StatusCode::OK);
    let headers = pub_detail["response_headers"].as_object().unwrap();
    assert_eq!(headers["set-cookie"], json!("<redacted>"));
    assert!(!headers
        .keys()
        .any(|k| k.to_ascii_lowercase().starts_with("x-hookbox-")));
}

#[tokio::test]
async fn share_row_invariant_masked_value_never_appears_elsewhere() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    add_rule(
        &app,
        &secret,
        &token,
        json!({
            "match": {"method":"GET","path":"/p"},
            "response": {"status_code": 200, "content_type":"text/plain","body_template":"ok"}
        }),
    )
    .await;
    let host = format!("{token}.mock.local");
    let req = Request::builder()
        .method("GET")
        .uri("/p?x=1")
        .header("host", &host)
        .header("authorization", "Bearer super-secret-value-12345")
        .body(Body::empty())
        .unwrap();
    let _ = app.clone().oneshot(req).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let list = list_requests(&app, &secret, &token).await;
    let request_id = list.as_array().unwrap()[0]["id"].as_i64().unwrap();
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    let (s, detail, _) = public_detail(&app, &code, request_id).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        detail["request_headers"]["authorization"],
        json!("<redacted>")
    );
    let raw = serde_json::to_string(&detail).unwrap();
    assert!(!raw.contains("super-secret-value-12345"));
}

/// Re-validates journey F4-viewer + PRD §5.5.4 / AC-77: `request_count` is a
/// monotonic lifetime counter, bumped on the same write path as the trace
/// (`repo::requests::insert_trace`), so it can never drift from the number of rows the
/// viewer (and the owner) actually see listed.
#[tokio::test]
async fn request_count_and_last_hit_increment_on_the_trace_write_path() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    add_rule(
        &app,
        &secret,
        &token,
        json!({"match":{"method":"GET","path":"/p"},"response":{"status_code":200}}),
    )
    .await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    // Before any mock request: zero, and last_hit is null.
    let (s, detail, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(detail["request_count"], json!(0));
    assert!(detail["last_hit"].is_null());

    // Serve N=3 mock requests against the endpoint.
    let host = format!("{token}.mock.local");
    for _ in 0..3 {
        let req = Request::builder()
            .method("GET")
            .uri("/p")
            .header("host", &host)
            .body(Body::empty())
            .unwrap();
        let _ = app.clone().oneshot(req).await.unwrap();
    }
    // Give the spawned trace task a moment (fire-and-forget, AC-73(a)).
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;

    // The owner-authenticated endpoint detail agrees...
    let (s, detail, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(detail["request_count"], json!(3));
    assert!(!detail["last_hit"].is_null());

    // ...and so does the PUBLIC share viewer, right next to the 3 listed
    // rows it renders underneath the subject line (the exact contradiction
    // this bug reported: "0 requests received in total" beside 3 rows).
    let (s, feed, _) = public_list(&app, &code, "").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(feed["endpoint"]["request_count"], json!(3));
    assert_eq!(feed["requests"].as_array().unwrap().len(), 3);
}

// hookbox-mun.36 — F4 channel C: the wildcard mock host IS `<token>.<MOCK_DOMAIN>`,
// so on a `default_mode = "echo"` row the persisted `response_body`'s `headers`
// sub-object used to carry the endpoint token verbatim via `host`/`origin`/
// `referer`. The public share projection filters `request_headers` and
// `response_headers` for exactly that set (AC-43, AC-S2, hookbox-mun.34) but
// passed `response_body` through untouched, so the token reappeared there.
// Re-validates AC-43 / journey.md F4 VIEWER step 6.
#[tokio::test]
async fn f4_channel_c_echo_response_body_has_no_token_or_mock_host_publicly() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    patch_endpoint(&app, &secret, &token, json!({"default_mode":"echo"})).await;
    let host = format!("{token}.mock.local");
    let req = Request::builder()
        .method("POST")
        .uri("/hooks/order")
        .header("host", &host)
        .header("origin", format!("https://{host}"))
        .header("referer", format!("https://{host}/somewhere"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let client_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let client_body: Value = serde_json::from_slice(&client_bytes).unwrap();
    // The §2 non-goal / AC-72: the CLIENT's own echo body is untouched — it
    // still carries the real host/origin/referer.
    assert_eq!(client_body["headers"]["host"], json!(host));
    assert_eq!(
        client_body["headers"]["origin"],
        json!(format!("https://{host}"))
    );

    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let detail = latest_request_detail(&app, &secret, &token).await;
    let request_id = detail["id"].as_i64().unwrap();
    let stored_raw = detail["response_body"].as_str().unwrap();
    // The fix is scoped to the echo payload's `headers` sub-object only: the
    // stored top-level `request_headers` column stays verbatim per §5.11 (it
    // is a different column, filtered only at the public projection).
    assert_eq!(detail["request_headers"]["host"], json!(host));
    let stored: Value = serde_json::from_str(stored_raw).unwrap();
    assert!(stored["headers"].get("host").is_none());
    assert!(stored["headers"].get("origin").is_none());
    assert!(stored["headers"].get("referer").is_none());
    assert!(
        !stored_raw.contains(&token),
        "persisted response_body must not carry the endpoint token: {stored_raw}"
    );

    let (_s, share_body, _) = mint_share(&app, &secret, &token, None).await;
    let code = share_body["code"].as_str().unwrap().to_string();
    let (s, public, _) = public_detail(&app, &code, request_id).await;
    assert_eq!(s, StatusCode::OK);
    let public_raw = serde_json::to_string(&public["response_body"]).unwrap();
    assert!(
        !public_raw.contains(&token),
        "public response_body must not carry the endpoint token: {public_raw}"
    );
    assert!(
        !public_raw.to_ascii_lowercase().contains("mock.local"),
        "public response_body must not carry the mock host: {public_raw}"
    );
}

/// Recursively collect every JSON leaf (string, bool, number, null) as its
/// string form. Used by the AC-S4 generic row-walk invariant below: "nothing
/// masked in `request_headers` appears verbatim anywhere else in the same
/// public row" is checked structurally, not by re-deriving the shape of one
/// specific channel.
fn collect_leaf_strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for val in m.values() {
                collect_leaf_strings(val, out);
            }
        }
        Value::Array(a) => {
            for val in a {
                collect_leaf_strings(val, out);
            }
        }
        Value::Null => {}
        Value::String(s) => out.push(s.clone()),
        other => out.push(other.to_string()),
    }
}

// hookbox-mun.37 + hookbox-mun.38 — F4 "channel D": a proxy header whose
// NAME is outside the fixed host/origin/referer drop list, but whose VALUE
// embeds the endpoint token (here CASE-FOLDED, as a real reverse proxy's
// documented-lowercase `$host` variable would deliver it via the
// near-universal `proxy_set_header X-Forwarded-Host $host;` snippet — Apache
// mod_proxy / Caddy / Traefik add this header by DEFAULT, no operator
// template and no attacker setup required) used to survive the
// `request_headers` name filter AND the echo-persist path untouched, then
// resurface verbatim inside the SAME row's `response_body`, handing an
// anonymous share viewer a write-capable endpoint token.
//
// This is deliberately a GENERIC AC-S4 row-walk invariant, not a
// name-specific regression: `x-forwarded-host` in case-folded form is a
// header/casing combination none of the channel A/B/C tests above exercise,
// so a hypothetical channel E cannot slip through the same way channel D
// did.
#[tokio::test]
async fn f4_channel_d_ac_s4_generic_row_walk_no_masked_value_leaks_elsewhere() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    patch_endpoint(&app, &secret, &token, json!({"default_mode":"echo"})).await;
    let host = format!("{token}.mock.local");
    let folded_host = host.to_ascii_lowercase();
    let req = Request::builder()
        .method("POST")
        .uri("/ingress")
        .header("host", &host)
        .header("x-forwarded-host", &folded_host)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let client_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let client_body: Value = serde_json::from_slice(&client_bytes).unwrap();
    // Sanity: the §2 non-goal / AC-72 — the CLIENT's own echo body is
    // untouched and really does carry the case-folded header back.
    assert_eq!(
        client_body["headers"]["x-forwarded-host"],
        json!(folded_host)
    );

    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let detail = latest_request_detail(&app, &secret, &token).await;
    let request_id = detail["id"].as_i64().unwrap();

    let (_s, share_body, _) = mint_share(&app, &secret, &token, None).await;
    let code = share_body["code"].as_str().unwrap().to_string();
    let (s, public, _) = public_detail(&app, &code, request_id).await;
    assert_eq!(s, StatusCode::OK);

    // The mechanism actually engaged: `x-forwarded-host` is not in the fixed
    // host/origin/referer name-drop list, so it must be MASKED (value
    // replaced), not silently absent.
    assert_eq!(
        public["request_headers"]["x-forwarded-host"],
        json!("<redacted>"),
        "a header outside the host/origin/referer name list, but carrying \
         the token by VALUE, must still be masked in request_headers"
    );

    // AC-S4, generic row-walk: the exact value masked above (the token's
    // own case-folded host form) must appear verbatim NOWHERE else in the
    // same public row — including inside response_body's echoed `headers`
    // sub-object, the precise shape of channel D.
    let mut leaves = Vec::new();
    collect_leaf_strings(&public, &mut leaves);
    for leaf in &leaves {
        assert!(
            !leaf.contains(&folded_host),
            "a value masked in request_headers ('{folded_host}') leaked \
             verbatim into another leaf of the same public row: {leaf}"
        );
    }
    // Belt-and-braces over the whole serialized row: the raw token never
    // appears anywhere, in ANY letter case (catches every other channel at
    // once, not just this one header).
    let row_raw = serde_json::to_string(&public).unwrap().to_ascii_lowercase();
    assert!(
        !row_raw.contains(&token.to_ascii_lowercase()),
        "endpoint token leaked (case-insensitively) somewhere in the public row: {row_raw}"
    );
}
