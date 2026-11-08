//! Shared integration-test harness: an in-process `build_app` over an
//! in-memory SQLite DB, configured from a fixed key -> value source (never
//! the process environment, so tests stay parallel-safe), plus thin helpers
//! over the management API. Each test binary uses a subset of these.
#![allow(dead_code)]

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use hookbox::config::Config;
use hookbox::db;
use hookbox::router::build_app;
use hookbox::state::AppState;

pub const APP_HOST: &str = "app.local";
pub const MOCK_DOMAIN: &str = "mock.local";

/// `(status, JSON body or Null, response headers)`.
pub type Reply = (StatusCode, Value, HeaderMap);

/// The integration-test `Config`: wildcard mock domain `mock.local`, app host
/// `app.local`, and a session rate limit high enough never to bind (the
/// `oneshot` harness has no `ConnectInfo`, so every mint in one app shares
/// the `session:unknown` bucket). No `PUBLIC_BASE_URL`, so the session
/// cookie is the plain-http `hookbox_key`. Everything else is the
/// production default.
pub fn test_config() -> Config {
    Config::from_source(|key| {
        let value = match key {
            "MOCK_DOMAIN" => MOCK_DOMAIN,
            "APP_HOST" => APP_HOST,
            "SESSION_RATE_LIMIT_PER_MIN" => "1000000",
            _ => return None,
        };
        Some(value.to_string())
    })
}

/// A running app plus one owner's key (`secret`, used as a Bearer).
pub struct TestApp {
    pub app: Router,
    pub state: AppState,
    pub secret: String,
}

impl TestApp {
    /// App over `test_config()` with one owner already created.
    pub async fn new() -> Self {
        Self::with_config(|_| {}).await
    }

    /// Like `new`, after `tweak` overrides the config.
    pub async fn with_config(tweak: impl FnOnce(&mut Config)) -> Self {
        let pool = db::pool(":memory:")
            .await
            .expect("open in-memory SQLite pool");
        db::migrate(&pool).await.expect("apply migrations");
        let mut cfg = test_config();
        tweak(&mut cfg);
        let state = AppState::new(pool, cfg);
        let app = build_app(state.clone());
        let secret = create_owner(&app).await;
        TestApp { app, state, secret }
    }
}

/// The session cookie name under `test_config()` (plain http, no `__Host-`).
pub const COOKIE_NAME: &str = "hookbox_key";

/// The SPA's CSRF header, as the browser client sends it.
pub const CSRF: (&str, &str) = ("x-hookbox", "1");

/// `POST /api/session` exactly as the SPA sends it (CSRF header, no body).
pub async fn mint_session(app: &Router) -> Reply {
    call_with(app, "POST", APP_HOST, "/api/session", &[CSRF], None).await
}

/// The owner key from a session response's `Set-Cookie`, if it set ours
/// (either the plain or the `__Host-` secure-mode name).
pub fn session_cookie_key(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|line| {
            let pair = line.split(';').next()?.trim();
            let pair = pair.strip_prefix("__Host-").unwrap_or(pair);
            let value = pair.strip_prefix(COOKIE_NAME)?.strip_prefix('=')?;
            Some(value.to_string()).filter(|v| !v.is_empty())
        })
}

/// `Cookie: hookbox_key=<key>` as a header pair.
pub fn cookie_header(key: &str) -> (&'static str, String) {
    ("cookie", format!("{COOKIE_NAME}={key}"))
}

/// THE one place integration tests mint an owner: a fresh anonymous owner
/// through the public session API, returning its owner key (read from the
/// HttpOnly cookie) for use as a Bearer.
pub async fn create_owner(app: &Router) -> String {
    let (status, body, headers) = mint_session(app).await;
    assert_eq!(status, StatusCode::CREATED, "owner creation: {body:?}");
    session_cookie_key(&headers).expect("mint sets the session cookie")
}

/// One request through the full app; a JSON body is sent as
/// `application/json`, and a non-JSON/empty response body reads as `Null`.
pub async fn call(
    app: &Router,
    method: &str,
    host: &str,
    path: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> Reply {
    let auth = bearer.map(|t| format!("Bearer {t}"));
    let headers: Vec<(&str, &str)> = auth
        .as_deref()
        .map(|a| vec![("authorization", a)])
        .unwrap_or_default();
    call_with(app, method, host, path, &headers, body).await
}

/// Like `call`, with arbitrary extra request headers (cookies, Origin, …).
pub async fn call_with(
    app: &Router,
    method: &str,
    host: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> Reply {
    let b = headers.iter().fold(
        Request::builder()
            .method(method)
            .uri(path)
            .header("host", host),
        |b, (k, v)| b.header(*k, *v),
    );
    let req = match body {
        Some(j) => b
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&j).unwrap()))
            .unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, v, headers)
}

/// Create an endpoint for `secret`'s owner; returns its token.
pub async fn new_endpoint(app: &Router, secret: &str) -> String {
    let (s, body, _) = call(
        app,
        "POST",
        APP_HOST,
        "/api/endpoints",
        Some(secret),
        Some(json!({"name":"e"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    body["token"].as_str().unwrap().to_string()
}

/// Create a rule on `token`; returns its id.
pub async fn add_rule(app: &Router, secret: &str, token: &str, rule: Value) -> i64 {
    let (s, body, _) = call(
        app,
        "POST",
        APP_HOST,
        &format!("/api/endpoints/{token}/rules"),
        Some(secret),
        Some(rule),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "rule create: {body:?}");
    body["id"].as_i64().unwrap()
}

/// `PATCH /api/endpoints/{token}` with `patch`.
pub async fn patch_endpoint(app: &Router, secret: &str, token: &str, patch: Value) -> Reply {
    let path = format!("/api/endpoints/{token}");
    call(app, "PATCH", APP_HOST, &path, Some(secret), Some(patch)).await
}

// --- F4 share links --------------------------------------------------------------

pub async fn mint_share(app: &Router, secret: &str, token: &str, label: Option<&str>) -> Reply {
    let body = match label {
        Some(l) => json!({ "label": l }),
        None => json!({}),
    };
    let path = format!("/api/endpoints/{token}/shares");
    call(app, "POST", APP_HOST, &path, Some(secret), Some(body)).await
}

pub async fn list_shares_of(app: &Router, secret: &str, token: &str) -> Reply {
    let path = format!("/api/endpoints/{token}/shares");
    call(app, "GET", APP_HOST, &path, Some(secret), None).await
}

pub async fn revoke_share(app: &Router, secret: &str, token: &str, id: i64) -> Reply {
    let path = format!("/api/endpoints/{token}/shares/{id}");
    call(app, "DELETE", APP_HOST, &path, Some(secret), None).await
}

pub async fn public_list(app: &Router, code: &str, query: &str) -> Reply {
    let path = if query.is_empty() {
        format!("/api/share/{code}/requests")
    } else {
        format!("/api/share/{code}/requests?{query}")
    };
    call(app, "GET", APP_HOST, &path, None, None).await
}

pub async fn public_detail(app: &Router, code: &str, id: i64) -> Reply {
    let path = format!("/api/share/{code}/requests/{id}");
    call(app, "GET", APP_HOST, &path, None, None).await
}

// --- trace reads -----------------------------------------------------------------

/// `GET /api/endpoints/{token}/requests` (newest first).
pub async fn list_requests(app: &Router, secret: &str, token: &str) -> Value {
    let path = format!("/api/endpoints/{token}/requests");
    call(app, "GET", APP_HOST, &path, Some(secret), None)
        .await
        .1
}

/// The full owner detail of the newest trace on `token`.
pub async fn latest_request_detail(app: &Router, secret: &str, token: &str) -> Value {
    request_detail_matching(app, secret, token, None).await
}

/// Like `latest_request_detail`, but when `expect_served_by` is `Some(..)` it
/// selects the row whose `served_by` matches instead of assuming the
/// highest-id row is the one under test. This makes assertions
/// order-independent when a test fires two mock requests back-to-back: their
/// fire-and-forget trace-insert `tokio::spawn`s (engine.rs) race each other,
/// so the row with the highest id is not deterministically the
/// most-recently-fired request. See hookbox-mun.33.
pub async fn request_detail_matching(
    app: &Router,
    secret: &str,
    token: &str,
    expect_served_by: Option<&str>,
) -> Value {
    let list = list_requests(app, secret, token).await;
    let items = list.as_array().unwrap();
    let request_id = match expect_served_by {
        Some(served_by) => items
            .iter()
            .find(|item| item["served_by"] == json!(served_by))
            .unwrap_or_else(|| {
                panic!("no request with served_by={served_by:?} found in {items:?}")
            })["id"]
            .as_i64()
            .unwrap(),
        None => items[0]["id"].as_i64().unwrap(),
    };
    let path = format!("/api/requests/{request_id}");
    call(app, "GET", APP_HOST, &path, Some(secret), None)
        .await
        .1
}
