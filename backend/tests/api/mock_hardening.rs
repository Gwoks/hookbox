//! Mock-plane hardening (AC-S14d): every P1 response carries the sandbox
//! CSP, owner-authored responses can never set/clear the session cookie,
//! the app origin (`/e/<token>` path-fallback) never sets ANY cookie, and
//! the incoming session cookie never reaches anything mock-side.

use axum::http::{HeaderMap, StatusCode};
use serde_json::{json, Value};

use crate::common::{
    add_rule, call, call_with, new_endpoint, patch_endpoint, TestApp, APP_HOST, COOKIE_NAME,
};

const CSP: &str = "sandbox allow-scripts allow-forms allow-popups allow-modals allow-downloads";

fn all(h: &HeaderMap, name: &str) -> Vec<String> {
    h.get_all(name)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect()
}

fn assert_sandboxed(h: &HeaderMap, label: &str) {
    assert!(
        all(h, "content-security-policy").iter().any(|v| v == CSP),
        "{label}: missing sandbox CSP in {h:?}"
    );
}

/// A rule on `path` answering 200 with extra response `headers`.
async fn rule_with_headers(
    app: &axum::Router,
    secret: &str,
    token: &str,
    path: &str,
    headers: Value,
) {
    add_rule(
        app,
        secret,
        token,
        json!({
            "match": {"method": "GET", "path": path},
            "response": {"status_code": 200, "content_type": "text/plain",
                         "headers": headers, "body_template": "ok"}
        }),
    )
    .await;
}

#[tokio::test]
async fn every_mock_response_carries_the_sandbox_csp() {
    let TestApp { app, secret, .. } =
        TestApp::with_config(|cfg| cfg.max_ingest_body_bytes = 16).await;
    let token = new_endpoint(&app, &secret).await;
    rule_with_headers(&app, &secret, &token, "/hit", json!({})).await;
    let wildcard = format!("{token}.mock.local");
    let e = |p: &str| format!("/e/{token}{p}");

    let (s, _, h) = call(&app, "GET", &wildcard, "/hit", None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_sandboxed(&h, "rule (wildcard)");
    let (_, _, h) = call(&app, "GET", APP_HOST, &e("/hit"), None, None).await;
    assert_sandboxed(&h, "rule (path-fallback)");
    let (s, _, h) = call(&app, "GET", &wildcard, "/nothing", None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_sandboxed(&h, "default no_match");
    let (s, _, h) = call(
        &app,
        "POST",
        &wildcard,
        "/big",
        None,
        Some(json!({"x": "y".repeat(64)})),
    )
    .await;
    assert_eq!(s, StatusCode::PAYLOAD_TOO_LARGE);
    assert_sandboxed(&h, "413");
    let preflight = [
        ("origin", "https://client.example"),
        ("access-control-request-method", "POST"),
    ];
    let (_, _, h) = call_with(&app, "OPTIONS", &wildcard, "/x", &preflight, None).await;
    assert_sandboxed(&h, "cors preflight");
    let (s, _, h) = call(&app, "GET", "ghosttoken.mock.local", "/x", None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_sandboxed(&h, "unknown endpoint");

    patch_endpoint(&app, &secret, &token, json!({"auto_crud": true})).await;
    let (s, _, h) = call(
        &app,
        "POST",
        &wildcard,
        "/items",
        None,
        Some(json!({"a": 1})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    assert_sandboxed(&h, "auto-crud");

    let path = format!("/api/endpoints/{token}");
    call(&app, "DELETE", APP_HOST, &path, Some(&secret), None).await;
    let (s, _, h) = call(&app, "GET", &wildcard, "/hit", None, None).await;
    assert_eq!(s, StatusCode::GONE);
    assert_sandboxed(&h, "410 gone");

    // The management plane is NOT sandboxed.
    let (_, _, h) = call(&app, "GET", APP_HOST, "/api/endpoints", Some(&secret), None).await;
    assert!(h.get("content-security-policy").is_none());
}

#[tokio::test]
async fn a_rules_own_csp_is_kept_alongside_the_sandbox() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let own = json!({"content-security-policy": "default-src 'none'"});
    rule_with_headers(&app, &secret, &token, "/csp", own).await;
    let (_, _, h) = call(
        &app,
        "GET",
        &format!("{token}.mock.local"),
        "/csp",
        None,
        None,
    )
    .await;
    assert_eq!(
        all(&h, "content-security-policy"),
        vec!["default-src 'none'".to_string(), CSP.to_string()]
    );
}

#[tokio::test]
async fn rules_cannot_set_the_session_cookie_or_clear_site_data() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let wildcard = format!("{token}.mock.local");
    let fixation = json!({"set-cookie": format!("{COOKIE_NAME}=attacker; Path=/")});
    rule_with_headers(&app, &secret, &token, "/fix", fixation).await;
    let host_fixation = json!({"set-cookie": "__Host-hookbox_key=attacker; Path=/; Secure"});
    rule_with_headers(&app, &secret, &token, "/fix2", host_fixation).await;
    let wipe = json!({"clear-site-data": "\"cookies\", \"storage\""});
    rule_with_headers(&app, &secret, &token, "/wipe", wipe).await;

    for path in ["/fix", "/fix2"] {
        let (s, _, h) = call(&app, "GET", &wildcard, path, None, None).await;
        assert_eq!(s, StatusCode::OK);
        assert!(h.get("set-cookie").is_none(), "{path}: {h:?}");
    }
    let (_, _, h) = call(&app, "GET", &wildcard, "/wipe", None, None).await;
    assert!(h.get("clear-site-data").is_none());
    let (_, _, h) = call(
        &app,
        "GET",
        APP_HOST,
        &format!("/e/{token}/wipe"),
        None,
        None,
    )
    .await;
    assert!(h.get("clear-site-data").is_none());
}

#[tokio::test]
async fn path_fallback_strips_every_set_cookie_but_wildcard_keeps_foreign_ones() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    rule_with_headers(
        &app,
        &secret,
        &token,
        "/c",
        json!({"set-cookie": "foo=bar"}),
    )
    .await;

    let (_, _, h) = call(&app, "GET", APP_HOST, &format!("/e/{token}/c"), None, None).await;
    assert!(
        h.get("set-cookie").is_none(),
        "app origin must not set cookies"
    );
    let (_, _, h) = call(
        &app,
        "GET",
        &format!("{token}.mock.local"),
        "/c",
        None,
        None,
    )
    .await;
    assert_eq!(all(&h, "set-cookie"), vec!["foo=bar".to_string()]);
}

#[tokio::test]
async fn the_session_cookie_never_reaches_echo_templates_or_traces() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    add_rule(
        &app,
        &secret,
        &token,
        json!({
            "match": {"method": "GET", "path": "/tpl"},
            "response": {"status_code": 200, "content_type": "application/json",
                         "body_template": "{\"cookie\":\"{{request.header.cookie}}\"}"}
        }),
    )
    .await;
    let cookie = format!("other=1; {COOKIE_NAME}={secret}");
    let sent = [("cookie", cookie.as_str())];

    let (s, body, _) = call_with(
        &app,
        "GET",
        APP_HOST,
        &format!("/e/{token}/tpl"),
        &sent,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["cookie"], json!("other=1"));

    patch_endpoint(&app, &secret, &token, json!({"default_mode": "echo"})).await;
    let (s, body, _) = call_with(
        &app,
        "GET",
        APP_HOST,
        &format!("/e/{token}/echo"),
        &sent,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["headers"]["cookie"], json!("other=1"));
    assert!(!body.to_string().contains(&secret));

    // Only our cookie: the Cookie header disappears entirely.
    let only = format!("{COOKIE_NAME}={secret}");
    let only = [("cookie", only.as_str())];
    let (_, body, _) = call_with(
        &app,
        "GET",
        APP_HOST,
        &format!("/e/{token}/echo"),
        &only,
        None,
    )
    .await;
    assert!(body["headers"].get("cookie").is_none(), "{body}");

    // Nothing persisted for the owner carries the key either.
    let path = format!("/api/endpoints/{token}/requests");
    let (_, list, _) = call(&app, "GET", APP_HOST, &path, Some(&secret), None).await;
    for item in list.as_array().unwrap() {
        let path = format!("/api/requests/{}", item["id"]);
        let (_, detail, _) = call(&app, "GET", APP_HOST, &path, Some(&secret), None).await;
        assert!(!detail.to_string().contains(&secret), "trace leaks the key");
    }
}
