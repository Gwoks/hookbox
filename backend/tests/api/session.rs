//! Anonymous owner sessions (AC-S14a): mint / restore / who-am-I / reveal /
//! rotate / sign-out, the cookie wire format, the CSRF guard on every
//! cookie-authenticated call, and the session rate limit.

use axum::http::{HeaderMap, StatusCode};
use serde_json::{json, Value};

use crate::common::{
    call, call_with, cookie_header, mint_session, mint_share, new_endpoint, public_list,
    session_cookie_key, Reply, TestApp, APP_HOST, CSRF,
};

fn set_cookies(h: &HeaderMap) -> Vec<String> {
    h.get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect()
}

fn assert_no_store(h: &HeaderMap) {
    assert_eq!(h.get("cache-control").unwrap(), "no-store");
}

/// Request headers for a cookie-authenticated SPA call.
fn spa(key: &str) -> Vec<(&'static str, String)> {
    vec![cookie_header(key), (CSRF.0, CSRF.1.to_string())]
}

fn refs<'a>(h: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    h.iter().map(|(k, v)| (*k, v.as_str())).collect()
}

async fn spa_call(app: &axum::Router, method: &str, path: &str, key: &str) -> Reply {
    call_with(app, method, APP_HOST, path, &refs(&spa(key)), None).await
}

async fn restore(app: &axum::Router, body: Value) -> Reply {
    call_with(
        app,
        "POST",
        APP_HOST,
        "/api/session/restore",
        &[CSRF],
        Some(body),
    )
    .await
}

// --- mint --------------------------------------------------------------------

#[tokio::test]
async fn mint_sets_strict_httponly_cookie_and_never_returns_the_key() {
    let TestApp { app, .. } = TestApp::new().await;
    let (s, body, h) = mint_session(&app).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_no_store(&h);
    assert_eq!(h.get("x-hookbox-plane").unwrap(), "api");

    let cookies = set_cookies(&h);
    assert_eq!(cookies.len(), 1);
    let key = session_cookie_key(&h).unwrap();
    assert_eq!(
        cookies[0],
        format!("hookbox_key={key}; Path=/; Max-Age=34560000; HttpOnly; SameSite=Strict")
    );
    assert!(key.len() >= 43);

    let obj = body.as_object().unwrap();
    let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["endpoints", "owner_id", "primary"]);
    assert!(
        !body.to_string().contains(&key),
        "key must not be in the body"
    );
    assert_eq!(body["owner_id"].as_str().unwrap().len(), 16);
    assert_eq!(body["endpoints"].as_array().unwrap().len(), 1);
    assert_eq!(body["primary"]["token"], body["endpoints"][0]["token"]);
}

#[tokio::test]
async fn secure_mode_uses_host_prefixed_secure_cookie() {
    let TestApp { app, .. } = TestApp::with_config(|cfg| cfg.session_cookie_secure = true).await;
    let (s, _, h) = mint_session(&app).await;
    assert_eq!(s, StatusCode::CREATED);
    let cookie = &set_cookies(&h)[0];
    assert!(cookie.starts_with("__Host-hookbox_key="), "{cookie}");
    assert!(
        cookie.ends_with("; HttpOnly; SameSite=Strict; Secure"),
        "{cookie}"
    );
    assert!(!cookie.contains("Domain"));

    // Only the configured name is read: the plain name is ignored.
    let key = session_cookie_key(&h).unwrap();
    let plain = format!("hookbox_key={key}");
    let headers = [("cookie", plain.as_str()), CSRF];
    let (s, _, _) = call_with(&app, "GET", APP_HOST, "/api/session", &headers, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let host = format!("__Host-hookbox_key={key}");
    let headers = [("cookie", host.as_str()), CSRF];
    let (s, _, _) = call_with(&app, "GET", APP_HOST, "/api/session", &headers, None).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn mint_without_csrf_header_or_from_foreign_origin_is_403() {
    let TestApp { app, .. } = TestApp::new().await;
    let cases: [&[(&str, &str)]; 5] = [
        &[],
        &[CSRF, ("origin", "null")],
        &[CSRF, ("origin", "https://evil.example")],
        &[CSRF, ("origin", "https://tok1234567.mock.local")],
        &[CSRF, ("sec-fetch-site", "cross-site")],
    ];
    for headers in cases {
        let (s, body, h) = call_with(&app, "POST", APP_HOST, "/api/session", headers, None).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{headers:?}");
        assert_eq!(body["error"], json!("csrf_rejected"));
        assert!(set_cookies(&h).is_empty());
        assert_no_store(&h);
    }
    // Same host on another port (vite dev server) is fine.
    let headers = [CSRF, ("origin", "http://app.local:5173")];
    let (s, _, _) = call_with(&app, "POST", APP_HOST, "/api/session", &headers, None).await;
    assert_eq!(s, StatusCode::CREATED);
}

#[tokio::test]
async fn mint_with_a_valid_cookie_returns_the_existing_owner() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let (_, me, _) = call(&app, "GET", APP_HOST, "/api/session", Some(&secret), None).await;
    let (s, body, h) = spa_call(&app, "POST", "/api/session", &secret).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["owner_id"], me["owner_id"]);
    assert!(set_cookies(&h).is_empty(), "no new owner, no new cookie");

    // An unknown cookie mints a fresh owner over it.
    let (s, body, h) = spa_call(&app, "POST", "/api/session", "stale-key-from-an-old-db").await;
    assert_eq!(s, StatusCode::CREATED);
    assert_ne!(body["owner_id"], me["owner_id"]);
    assert!(session_cookie_key(&h).is_some());
}

// --- restore -----------------------------------------------------------------

#[tokio::test]
async fn restore_sets_the_cookie_without_rotating() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let (s, body, h) = restore(&app, json!({ "key": secret })).await;
    assert_eq!(s, StatusCode::OK);
    assert_no_store(&h);
    assert_eq!(session_cookie_key(&h).as_deref(), Some(secret.as_str()));
    assert_eq!(body["endpoints"].as_array().unwrap().len(), 1);
    assert!(!body.to_string().contains(&secret));

    // The restored key keeps working everywhere (no rotation).
    let (s, _, _) = call(&app, "GET", APP_HOST, "/api/endpoints", Some(&secret), None).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn restore_malformed_or_unknown_key_is_401_without_cookie() {
    let TestApp { app, .. } = TestApp::new().await;
    for body in [
        json!({ "key": "short" }),
        json!({ "key": "has spaces in it which is not base64url" }),
        json!({ "key": 42 }),
        json!({}),
        json!({ "key": "A".repeat(43) }),
    ] {
        let (s, resp, h) = restore(&app, body.clone()).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED, "{body}");
        assert_eq!(resp["error"], json!("invalid_key"));
        assert!(set_cookies(&h).is_empty());
        assert_no_store(&h);
    }
    let (s, body, _) = call(&app, "POST", APP_HOST, "/api/session/restore", None, None).await;
    assert_eq!(s, StatusCode::FORBIDDEN, "restore is CSRF-guarded: {body}");
}

// --- who am I / owner routes by cookie -------------------------------------------

#[tokio::test]
async fn get_session_by_cookie_refreshes_it_and_needs_the_csrf_header() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let (s, body, h) = spa_call(&app, "GET", "/api/session", &secret).await;
    assert_eq!(s, StatusCode::OK);
    assert_no_store(&h);
    assert_eq!(session_cookie_key(&h).as_deref(), Some(secret.as_str()));
    assert!(body["primary"]["token"].is_string());

    let (s, body, h) = call(&app, "GET", APP_HOST, "/api/session", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], json!("unauthorized"));
    assert_no_store(&h);

    let cookie_only = [cookie_header(&secret)];
    let (s, body, h) = call_with(
        &app,
        "GET",
        APP_HOST,
        "/api/session",
        &refs(&cookie_only),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], json!("csrf_rejected"));
    assert_no_store(&h);

    // A Bearer caller gets the summary but never a cookie.
    let (s, _, h) = call(&app, "GET", APP_HOST, "/api/session", Some(&secret), None).await;
    assert_eq!(s, StatusCode::OK);
    assert!(set_cookies(&h).is_empty());
}

#[tokio::test]
async fn session_summary_excludes_tombstoned_endpoints() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let (_, me, _) = call(&app, "GET", APP_HOST, "/api/session", Some(&secret), None).await;
    let primary = me["primary"]["token"].as_str().unwrap().to_string();
    let path = format!("/api/endpoints/{primary}");
    call(&app, "DELETE", APP_HOST, &path, Some(&secret), None).await;
    let (s, me, _) = call(&app, "GET", APP_HOST, "/api/session", Some(&secret), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(me["endpoints"], json!([]));
    assert!(me["primary"].is_null());

    let second = new_endpoint(&app, &secret).await;
    let (_, me, _) = call(&app, "GET", APP_HOST, "/api/session", Some(&secret), None).await;
    assert_eq!(me["primary"]["token"], json!(second));
}

#[tokio::test]
async fn owner_routes_accept_the_cookie_only_with_the_csrf_header() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let cookie_only = [cookie_header(&secret)];
    let (s, body, _) = call_with(
        &app,
        "GET",
        APP_HOST,
        "/api/endpoints",
        &refs(&cookie_only),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], json!("csrf_rejected"));
    let (s, body, _) = spa_call(&app, "GET", "/api/endpoints", &secret).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 1);
    // A wrong Bearer is final even beside a valid cookie.
    let spa_headers = spa(&secret);
    let mut headers = refs(&spa_headers);
    headers.push(("authorization", "Bearer not-the-key"));
    let (s, _, _) = call_with(&app, "GET", APP_HOST, "/api/endpoints", &headers, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn public_share_routes_need_no_csrf_header() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let (s, share, _) = mint_share(&app, &secret, &token, None).await;
    assert_eq!(s, StatusCode::CREATED);
    let code = share["code"].as_str().unwrap();
    let (s, _, _) = public_list(&app, code, "").await;
    assert_eq!(s, StatusCode::OK);
}

// --- reveal / rotate / sign-out ------------------------------------------------

#[tokio::test]
async fn reveal_returns_the_presented_key() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let (s, body, h) = spa_call(&app, "POST", "/api/session/key", &secret).await;
    assert_eq!(s, StatusCode::OK);
    assert_no_store(&h);
    assert_eq!(body, json!({ "key": secret }));
    let (s, body, _) = call(
        &app,
        "POST",
        APP_HOST,
        "/api/session/key",
        Some(&secret),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["key"], json!(secret));
    let (s, _, _) = call(&app, "POST", APP_HOST, "/api/session/key", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rotate_invalidates_the_old_key_and_revokes_tunnels() {
    let TestApp { app, state, secret } = TestApp::new().await;
    let (_, me, _) = call(&app, "GET", APP_HOST, "/api/session", Some(&secret), None).await;
    let token = me["primary"]["token"].as_str().unwrap().to_string();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    state.tunnels.bind(&token, tx);
    assert!(state.tunnels.is_active(&token));

    let (s, body, h) = spa_call(&app, "POST", "/api/session/rotate", &secret).await;
    assert_eq!(s, StatusCode::OK);
    assert_no_store(&h);
    let new_key = body["key"].as_str().unwrap().to_string();
    assert_ne!(new_key, secret);
    assert_eq!(session_cookie_key(&h).as_deref(), Some(new_key.as_str()));
    assert!(
        !state.tunnels.is_active(&token),
        "rotation drops the tunnel"
    );

    let (s, _, _) = call(&app, "GET", APP_HOST, "/api/endpoints", Some(&secret), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _, _) = spa_call(&app, "GET", "/api/endpoints", &secret).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _, _) = call(
        &app,
        "GET",
        APP_HOST,
        "/api/endpoints",
        Some(&new_key),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);

    // A Bearer rotation returns the key but sets no cookie.
    let (s, body, h) = call(
        &app,
        "POST",
        APP_HOST,
        "/api/session/rotate",
        Some(&new_key),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert!(body["key"].is_string());
    assert!(set_cookies(&h).is_empty());
}

#[tokio::test]
async fn sign_out_clears_the_cookie_but_the_key_stays_valid() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let (s, body, h) = spa_call(&app, "DELETE", "/api/session", &secret).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert!(body.is_null());
    assert_no_store(&h);
    assert_eq!(
        set_cookies(&h),
        vec!["hookbox_key=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict".to_string()]
    );
    let (s, _, _) = call(&app, "GET", APP_HOST, "/api/endpoints", Some(&secret), None).await;
    assert_eq!(s, StatusCode::OK);

    let cookie_only = [cookie_header(&secret)];
    let (s, _, h) = call_with(
        &app,
        "DELETE",
        APP_HOST,
        "/api/session",
        &refs(&cookie_only),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN, "sign-out is CSRF-guarded");
    assert!(set_cookies(&h).is_empty());
}

// --- rate limit ----------------------------------------------------------------

#[tokio::test]
async fn mint_and_restore_share_the_session_rate_limit() {
    // The fixture's own mint is the first of the two allowed per minute.
    let TestApp { app, secret, .. } =
        TestApp::with_config(|cfg| cfg.session_rate_limit_per_min = 2).await;
    let (s, _, _) = mint_session(&app).await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, body, h) = mint_session(&app).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"], json!("rate_limited"));
    assert!(h.get("retry-after").is_some());
    assert_no_store(&h);
    let (s, _, _) = restore(&app, json!({ "key": secret })).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    // Owner routes are not session-limited.
    let (s, _, _) = call(&app, "GET", APP_HOST, "/api/session", Some(&secret), None).await;
    assert_eq!(s, StatusCode::OK);
}
