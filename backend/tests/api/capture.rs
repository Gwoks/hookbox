//! F7 response-body capture: every `served_by` path persists the exact bytes
//! the client received (lossy UTF-8, capped, NULL when empty), the echo copy is
//! redacted at rest, and the feed/detail shapes are unchanged.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::common::{
    add_rule, call, latest_request_detail, list_requests, new_endpoint, patch_endpoint,
    request_detail_matching, TestApp,
};

/// Spin up a tiny real HTTP upstream on an ephemeral loopback port, returning
/// `bytes` verbatim for every request. Used to exercise the MITM `served_by`
/// path end-to-end (real bytes crossing a real socket), rather than only unit
/// testing `capture_response_body` in isolation.
async fn spawn_test_upstream_bytes(bytes: &'static [u8]) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = axum::Router::new().fallback(move || async move {
        axum::response::Response::builder()
            .status(200)
            .header("content-type", "application/octet-stream")
            .body(Body::from(bytes))
            .unwrap()
    });
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

/// Build a fresh app with `mitm_allow_private = true` (the SSRF guard would
/// otherwise refuse a loopback target) — needed for every MITM-path test.
async fn app_with_mitm_allowed() -> (axum::Router, String) {
    let TestApp { app, secret, .. } = TestApp::with_config(|cfg| {
        cfg.mitm_allow_private = true;
    })
    .await;
    (app, secret)
}

/// Build a fresh app with a raised `template_max_size` — F7's persist cap
/// (`max_body_bytes`, default 256_000) and the RULE VALIDATION cap
/// (`template_max_size`, ALSO 256_000 by default) happen to share a default
/// value, so a `body_template` big enough to exercise truncation on the
/// persist side would be rejected by rule creation itself unless this is
/// raised. `max_body_bytes` (the value under test) is left at its default.
async fn app_with_larger_template_cap() -> (axum::Router, String) {
    let TestApp { app, secret, .. } = TestApp::with_config(|cfg| {
        cfg.template_max_size = 400_000;
    })
    .await;
    (app, secret)
}

#[tokio::test]
async fn f7_rule_response_body_captured_exactly() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    add_rule(
        &app,
        &secret,
        &token,
        json!({
            "match": {"method":"GET","path":"/p"},
            "response": {"status_code": 201, "content_type":"text/plain","body_template":"hello-rule-body"}
        }),
    )
    .await;
    let host = format!("{token}.mock.local");
    call(&app, "GET", &host, "/p", None, None).await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let detail = latest_request_detail(&app, &secret, &token).await;
    assert_eq!(detail["served_by"], json!("rule"));
    assert_eq!(detail["response_body"], json!("hello-rule-body"));
}

#[tokio::test]
async fn f7_cors_preflight_response_body_is_null() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await; // cors_enabled defaults true
    let host = format!("{token}.mock.local");
    let req = Request::builder()
        .method("OPTIONS")
        .uri("/x")
        .header("host", &host)
        .header("origin", "https://x.io")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let detail = latest_request_detail(&app, &secret, &token).await;
    assert_eq!(detail["served_by"], json!("cors"));
    assert!(detail["response_body"].is_null()); // AC-69: zero-length body -> NULL
}

#[tokio::test]
async fn f7_ratelimit_429_response_body_captured() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    patch_endpoint(&app, &secret, &token, json!({"rate_limit_per_min": 1})).await;
    let host = format!("{token}.mock.local");
    call(&app, "GET", &host, "/x", None, None).await; // consumes the one token
    let (s, _b, _) = call(&app, "GET", &host, "/x", None, None).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    // The two mock requests above are fired back-to-back with no settle in between, so their
    // fire-and-forget trace inserts (engine.rs) can land in either order. Select the row by
    // served_by rather than by recency so the assertion is order-independent (hookbox-mun.33).
    let detail = request_detail_matching(&app, &secret, &token, Some("ratelimit")).await;
    assert_eq!(detail["served_by"], json!("ratelimit"));
    let stored = detail["response_body"].as_str().unwrap();
    assert!(stored.contains("rate_limited"));
}

#[tokio::test]
async fn f7_chaos_status_captured_and_chaos_drop_is_null() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    patch_endpoint(
        &app,
        &secret,
        &token,
        json!({"chaos_pct": 100, "chaos_mode": "error"}),
    )
    .await;
    let host = format!("{token}.mock.local");
    let (s, _b, _) = call(&app, "GET", &host, "/x", None, None).await;
    assert!(s.is_server_error());
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let detail = latest_request_detail(&app, &secret, &token).await;
    assert_eq!(detail["served_by"], json!("chaos"));
    assert!(detail["response_body"].as_str().unwrap().contains("chaos"));

    // Dropout: the pre-existing low-fidelity row (status_code 0, {} headers)
    // is deliberately NOT fixed by F7 (R-DROPOUT) — response_body stays NULL.
    patch_endpoint(&app, &secret, &token, json!({"chaos_mode": "dropout"})).await;
    let req = Request::builder()
        .method("GET")
        .uri("/y")
        .header("host", &host)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 499);
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let detail2 = latest_request_detail(&app, &secret, &token).await;
    assert_eq!(detail2["served_by"], json!("chaos"));
    assert_eq!(detail2["status_code"], json!(0));
    assert!(detail2["response_body"].is_null());
}

#[tokio::test]
async fn f7_crud_post_captured_and_delete_204_is_null() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    patch_endpoint(&app, &secret, &token, json!({"auto_crud": true})).await;
    let host = format!("{token}.mock.local");
    let (s, body, _) = call(
        &app,
        "POST",
        &host,
        "/books",
        None,
        Some(json!({"title":"Dune"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    let id = body["id"].as_str().unwrap().to_string();
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let post_detail = latest_request_detail(&app, &secret, &token).await;
    assert_eq!(post_detail["served_by"], json!("crud"));
    let stored: Value =
        serde_json::from_str(post_detail["response_body"].as_str().unwrap()).unwrap();
    assert_eq!(stored["title"], json!("Dune"));

    let (s, _b, _) = call(&app, "DELETE", &host, &format!("/books/{id}"), None, None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let delete_detail = latest_request_detail(&app, &secret, &token).await;
    assert!(delete_detail["response_body"].is_null()); // AC-69: empty 204 CRUD
}

#[tokio::test]
async fn f7_default_mock_404_response_body_captured() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let host = format!("{token}.mock.local");
    let (s, _b, _) = call(&app, "GET", &host, "/nope", None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let detail = latest_request_detail(&app, &secret, &token).await;
    assert_eq!(detail["served_by"], json!("default"));
    let stored: Value = serde_json::from_str(detail["response_body"].as_str().unwrap()).unwrap();
    assert_eq!(stored["error"], json!("no_match"));
}

#[tokio::test]
async fn f7_default_echo_response_body_redacted_at_rest_while_client_gets_raw() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    patch_endpoint(&app, &secret, &token, json!({"default_mode":"echo"})).await;
    let host = format!("{token}.mock.local");
    let req = Request::builder()
        .method("GET")
        .uri("/echo-me")
        .header("host", &host)
        .header("authorization", "Bearer super-secret-echo-value")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let client_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let client_body: Value = serde_json::from_slice(&client_bytes).unwrap();
    // AC-72 / the F7 non-goal: the CLIENT still sees the real header value.
    assert_eq!(
        client_body["headers"]["authorization"],
        json!("Bearer super-secret-echo-value")
    );

    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let detail = latest_request_detail(&app, &secret, &token).await;
    let stored_raw = detail["response_body"].as_str().unwrap();
    let stored: Value = serde_json::from_str(stored_raw).unwrap();
    // AC-S3: the PERSISTED copy is redacted even though the client's was not.
    assert_eq!(stored["headers"]["authorization"], json!("<redacted>"));
    assert!(!stored_raw.contains("super-secret-echo-value"));
}

#[tokio::test]
async fn f7_mitm_response_body_captured_from_real_upstream() {
    let (app, secret) = app_with_mitm_allowed().await;
    let token = new_endpoint(&app, &secret).await;
    let upstream = spawn_test_upstream_bytes(b"upstream-body-xyz").await;
    patch_endpoint(&app, &secret, &token, json!({"target_url": upstream})).await;

    let host = format!("{token}.mock.local");
    let (s, _b, h) = call(&app, "GET", &host, "/anything", None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h.get("x-hookbox-served-by").unwrap(), "mitm");

    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let detail = latest_request_detail(&app, &secret, &token).await;
    assert_eq!(detail["served_by"], json!("mitm"));
    assert_eq!(detail["response_body"], json!("upstream-body-xyz"));
}

#[tokio::test]
async fn f7_lossy_utf8_response_body_client_gets_raw_bytes_stored_is_lossy() {
    let (app, secret) = app_with_mitm_allowed().await;
    let token = new_endpoint(&app, &secret).await;
    // 'a' + three invalid UTF-8 continuation/lead bytes + 'b'.
    const RAW: &[u8] = &[0x61, 0x80, 0xFF, 0xFE, 0x62];
    let upstream = spawn_test_upstream_bytes(RAW).await;
    patch_endpoint(&app, &secret, &token, json!({"target_url": upstream})).await;

    let host = format!("{token}.mock.local");
    let req = Request::builder()
        .method("GET")
        .uri("/x")
        .header("host", &host)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let client_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    // AC-72: the client receives the ORIGINAL raw bytes, untouched.
    assert_eq!(client_bytes.as_ref(), RAW);

    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    let detail = latest_request_detail(&app, &secret, &token).await;
    let stored = detail["response_body"].as_str().unwrap();
    // AC-71: the TEXT column always holds valid UTF-8 — lossy-decoded from
    // the raw wire bytes.
    assert_eq!(stored, String::from_utf8_lossy(RAW).into_owned());
    assert!(stored.contains('\u{FFFD}'));
}

#[tokio::test]
async fn f7_response_body_truncated_to_cap_no_marker_and_exact_cap_stored_whole() {
    let (app, secret) = app_with_larger_template_cap().await;
    let token = new_endpoint(&app, &secret).await;
    let cap = 256_000usize; // MAX_BODY_BYTES default
    let host = format!("{token}.mock.local");

    // (a) A 300_000-byte ASCII body -> the CLIENT is unaffected by F7, but the
    // STORED value is cut to exactly `cap`, with no marker/flag appended.
    let big = "a".repeat(300_000);
    add_rule(
        &app,
        &secret,
        &token,
        json!({
            "match": {"method":"GET","path":"/big"},
            "response": {"status_code": 200, "content_type":"text/plain", "body_template": big}
        }),
    )
    .await;
    let req = Request::builder()
        .method("GET")
        .uri("/big")
        .header("host", &host)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let client_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(client_bytes.len(), 300_000);
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let detail = latest_request_detail(&app, &secret, &token).await;
    let stored = detail["response_body"].as_str().unwrap();
    assert_eq!(stored.len(), cap);
    assert!(stored.chars().all(|c| c == 'a'));

    // (c) A body of EXACTLY `cap` bytes is stored whole (not cut short).
    let exact = "b".repeat(cap);
    add_rule(
        &app,
        &secret,
        &token,
        json!({
            "match": {"method":"GET","path":"/exact"},
            "response": {"status_code": 200, "content_type":"text/plain", "body_template": exact}
        }),
    )
    .await;
    let req = Request::builder()
        .method("GET")
        .uri("/exact")
        .header("host", &host)
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let detail2 = latest_request_detail(&app, &secret, &token).await;
    assert_eq!(detail2["response_body"].as_str().unwrap().len(), cap);
}

#[tokio::test]
async fn f7_response_body_truncation_backs_off_at_multibyte_boundary_no_panic() {
    let (app, secret) = app_with_larger_template_cap().await;
    let token = new_endpoint(&app, &secret).await;
    let cap = 256_000usize;
    // A 3-byte '€' begins exactly at byte offset `cap - 1`, straddling the cap.
    let mut body = "a".repeat(cap - 1);
    body.push('€');
    body.push_str("tail");
    add_rule(
        &app,
        &secret,
        &token,
        json!({
            "match": {"method":"GET","path":"/mid"},
            "response": {"status_code": 200, "content_type":"text/plain", "body_template": body}
        }),
    )
    .await;
    let host = format!("{token}.mock.local");
    let req = Request::builder()
        .method("GET")
        .uri("/mid")
        .header("host", &host)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert!(
        resp.status().is_success(),
        "no panic; a normal 2xx (AC-S17)"
    );
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let list = list_requests(&app, &secret, &token).await;
    assert!(
        !list.as_array().unwrap().is_empty(),
        "a trace row must still be written even at the multibyte boundary (AC-S17)"
    );
    let detail = latest_request_detail(&app, &secret, &token).await;
    let stored = detail["response_body"].as_str().unwrap();
    let k = cap - stored.len();
    assert!(
        (1..=3).contains(&k),
        "expected MAX_BODY_BYTES - k with k<=3, got k={k}"
    );
    assert_eq!(stored, "a".repeat(cap - 1));
}

#[tokio::test]
async fn f7_request_body_truncated_through_same_helper() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    add_rule(
        &app,
        &secret,
        &token,
        json!({
            "match": {"method":"POST","path":"/echo2"},
            "response": {"status_code": 200, "content_type":"text/plain","body_template":"ok"}
        }),
    )
    .await;
    let host = format!("{token}.mock.local");
    let cap = 256_000usize;
    let big_req_body = "z".repeat(cap + 500);
    let req = Request::builder()
        .method("POST")
        .uri("/echo2")
        .header("host", &host)
        .header("content-type", "text/plain")
        .body(Body::from(big_req_body))
        .unwrap();
    app.clone().oneshot(req).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let detail = latest_request_detail(&app, &secret, &token).await;
    assert_eq!(detail["request_body"].as_str().unwrap().len(), cap);
}

#[tokio::test]
async fn f7_new_request_feed_payload_has_no_body_keys() {
    let TestApp { app, state, secret } = TestApp::new().await;
    let feed_hub = state.feed_hub.clone();
    let token = new_endpoint(&app, &secret).await;
    add_rule(
        &app,
        &secret,
        &token,
        json!({
            "match": {"method":"GET","path":"/p"},
            "response": {"status_code": 200, "content_type":"text/plain","body_template":"hello"}
        }),
    )
    .await;

    let mut sub = feed_hub.subscribe(&token);
    let host = format!("{token}.mock.local");
    call(&app, "GET", &host, "/p", None, None).await;

    let ev = tokio::time::timeout(std::time::Duration::from_secs(2), sub.rx.recv())
        .await
        .expect("feed event within timeout")
        .unwrap();
    assert_eq!(ev.kind, "new_request");
    let obj = ev.data.as_object().unwrap();
    assert!(!obj.contains_key("request_body"));
    assert!(!obj.contains_key("response_body"));
    assert_eq!(obj["served_by"], json!("rule")); // AC-74: summary fields unchanged
}

#[tokio::test]
async fn f7_request_detail_key_set_unchanged() {
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
    call(&app, "GET", &host, "/p", None, None).await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let detail = latest_request_detail(&app, &secret, &token).await;
    let keys: std::collections::BTreeSet<&str> = detail
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();
    let expected: std::collections::BTreeSet<&str> = [
        "id",
        "token",
        "method",
        "path",
        "status_code",
        "served_by",
        "matched_rule_id",
        "duration_ms",
        "overhead_ms",
        "timestamp",
        "request_headers",
        "query_params",
        "request_body",
        "response_headers",
        "response_body",
        "trace",
        "state_snapshot",
    ]
    .into_iter()
    .collect();
    assert_eq!(keys, expected, "AC-74: no new endpoint/model field for F7");
}
