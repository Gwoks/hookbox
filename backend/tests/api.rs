//! Integration tests over the in-process app (§5 parity guard, AC-55).
//!
//! Mirrors the reference `../shortener-link/backend/tests/` shape: build the real
//! `router::build_app(state)` over an in-memory SQLite DB and drive it with
//! `tower::ServiceExt::oneshot`. Covers the §5.2 management routes, §5.5
//! resolution order + conditions wrap + X-HookBox headers + 404-vs-410, §5.5
//! Auto-CRUD lifecycle, §5.5 Auto-CORS, §5.7 templating sandbox, and the flat
//! error envelope.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use hookbox::config::Config;
use hookbox::db;
use hookbox::ids;
use hookbox::router::build_app;
use hookbox::state::AppState;

async fn app() -> (axum::Router, String) {
    std::env::set_var("MOCK_DOMAIN", "mock.local");
    // The session anti-enumeration limiter (routes::api::session_rate_limited)
    // is a single process-wide static bucket keyed by client IP, and every
    // test in this binary that calls `Config::from_env()` shares it (there is
    // no real ConnectInfo in the `oneshot` harness, so ALL of them collapse
    // onto the "unknown" bucket). With ~35 tests in this file each minting at
    // least one session, the default 30/min default would intermittently 429
    // unrelated tests. Raise it here, in every place this file builds a
    // `Config`, so the shared bucket never binds — this does not change any
    // production default (the env var is scoped to this test binary process).
    std::env::set_var("SESSION_RATE_LIMIT_PER_MIN", "1000000");
    std::env::set_var("APP_HOST", "app.local");
    let pool = db::pool(":memory:").await.unwrap();
    db::migrate(&pool).await.unwrap();
    let state = AppState::new(pool, Config::from_env());
    let app = build_app(state);
    // Create an owner via /api/session.
    let (_s, body, _) = call(
        &app,
        "POST",
        "app.local",
        "/api/session",
        None,
        Some(json!({"email":"t@e.com"})),
    )
    .await;
    let secret = body["owner_secret"].as_str().unwrap().to_string();
    (app, secret)
}

async fn call(
    app: &axum::Router,
    method: &str,
    host: &str,
    path: &str,
    bearer: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value, axum::http::HeaderMap) {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header("host", host);
    if let Some(t) = bearer {
        b = b.header("authorization", format!("Bearer {t}"));
    }
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

async fn new_endpoint(app: &axum::Router, secret: &str) -> String {
    let (s, body, _) = call(
        app,
        "POST",
        "app.local",
        "/api/endpoints",
        Some(secret),
        Some(json!({"name":"e"})),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED);
    body["token"].as_str().unwrap().to_string()
}

async fn add_rule(app: &axum::Router, secret: &str, token: &str, rule: Value) -> i64 {
    let (s, body, _) = call(
        app,
        "POST",
        "app.local",
        &format!("/api/endpoints/{token}/rules"),
        Some(secret),
        Some(rule),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "rule create: {body:?}");
    body["id"].as_i64().unwrap()
}

// --- §5.2 management surface --------------------------------------------------

#[tokio::test]
async fn mock_url_uses_public_base_url_in_path_fallback_mode() {
    // Explicit Config override (no env mutation — parallel tests also read
    // Config::from_env()).
    std::env::set_var("MOCK_DOMAIN", "mock.local");
    // The session anti-enumeration limiter (routes::api::session_rate_limited)
    // is a single process-wide static bucket keyed by client IP, and every
    // test in this binary that calls `Config::from_env()` shares it (there is
    // no real ConnectInfo in the `oneshot` harness, so ALL of them collapse
    // onto the "unknown" bucket). With ~35 tests in this file each minting at
    // least one session, the default 30/min default would intermittently 429
    // unrelated tests. Raise it here, in every place this file builds a
    // `Config`, so the shared bucket never binds — this does not change any
    // production default (the env var is scoped to this test binary process).
    std::env::set_var("SESSION_RATE_LIMIT_PER_MIN", "1000000");
    std::env::set_var("APP_HOST", "app.local");
    let pool = db::pool(":memory:").await.unwrap();
    db::migrate(&pool).await.unwrap();
    let mut cfg = Config::from_env();
    cfg.path_fallback_only = true;
    cfg.public_base_url = "https://hookbox.example.com".to_string();
    let state = AppState::new(pool, cfg);
    let app = build_app(state);

    let (s, body, _) = call(
        &app,
        "POST",
        "app.local",
        "/api/session",
        None,
        Some(json!({"email":"base@e.com"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let ep = &body["primary"];
    let token = ep["token"].as_str().unwrap();
    assert_eq!(
        ep["mock_url"],
        json!(format!("https://hookbox.example.com/e/{token}"))
    );
    assert_eq!(
        ep["path_url"],
        json!(format!("https://hookbox.example.com/e/{token}"))
    );
}

#[tokio::test]
async fn mock_url_stays_relative_in_path_fallback_mode_without_base_url() {
    std::env::set_var("MOCK_DOMAIN", "mock.local");
    // The session anti-enumeration limiter (routes::api::session_rate_limited)
    // is a single process-wide static bucket keyed by client IP, and every
    // test in this binary that calls `Config::from_env()` shares it (there is
    // no real ConnectInfo in the `oneshot` harness, so ALL of them collapse
    // onto the "unknown" bucket). With ~35 tests in this file each minting at
    // least one session, the default 30/min default would intermittently 429
    // unrelated tests. Raise it here, in every place this file builds a
    // `Config`, so the shared bucket never binds — this does not change any
    // production default (the env var is scoped to this test binary process).
    std::env::set_var("SESSION_RATE_LIMIT_PER_MIN", "1000000");
    std::env::set_var("APP_HOST", "app.local");
    let pool = db::pool(":memory:").await.unwrap();
    db::migrate(&pool).await.unwrap();
    let mut cfg = Config::from_env();
    cfg.path_fallback_only = true;
    cfg.public_base_url = String::new();
    let state = AppState::new(pool, cfg);
    let app = build_app(state);

    let (s, body, _) = call(
        &app,
        "POST",
        "app.local",
        "/api/session",
        None,
        Some(json!({"email":"rel@e.com"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let ep = &body["primary"];
    let token = ep["token"].as_str().unwrap();
    assert_eq!(ep["mock_url"], json!(format!("/e/{token}")));
    assert_eq!(ep["path_url"], json!(format!("/e/{token}")));
}

#[tokio::test]
async fn session_shape_and_flat_error_envelope() {
    let (app, _secret) = app().await;
    // Unauthed /api -> 401 flat envelope + WWW-Authenticate.
    let (s, body, h) = call(&app, "GET", "app.local", "/api/endpoints", None, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], json!("unauthorized"));
    assert!(body["detail"].is_string());
    assert_eq!(h.get("www-authenticate").unwrap(), "Bearer");
    // bad email -> 422.
    let (s, _b, _) = call(
        &app,
        "POST",
        "app.local",
        "/api/session",
        None,
        Some(json!({"email":"nope"})),
    )
    .await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn rules_ordered_and_204_delete() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    let r1 = add_rule(
        &app,
        &secret,
        &token,
        json!({"priority": 50, "match": {"method":"GET"}}),
    )
    .await;
    add_rule(&app, &secret, &token, json!({"priority": 10})).await;
    let (s, list, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/endpoints/{token}/rules"),
        Some(&secret),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let arr = list.as_array().unwrap();
    assert_eq!(arr[0]["priority"], json!(10)); // priority,id order
                                               // DELETE -> 204 no body.
    let (s, body, _) = call(
        &app,
        "DELETE",
        "app.local",
        &format!("/api/endpoints/{token}/rules/{r1}"),
        Some(&secret),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert!(body.is_null());
}

// --- §5.5 resolution + X-HookBox headers + 404/410 ----------------------------

#[tokio::test]
async fn mock_rule_match_renders_template_and_identifies() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    add_rule(&app, &secret, &token, json!({
        "match": {"method": "GET", "path": "/users/:id"},
        "response": {"status_code": 200, "content_type":"application/json",
                     "body_template": "{\"id\":\"{{request.path.id}}\",\"m\":\"{{request.method}}\"}"}
    })).await;
    let host = format!("{token}.mock.local");
    let (s, body, h) = call(&app, "GET", &host, "/users/42", None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["id"], json!("42"));
    assert_eq!(body["m"], json!("GET"));
    assert_eq!(h.get("x-hookbox-endpoint").unwrap(), token.as_str());
    assert_eq!(h.get("x-hookbox-served-by").unwrap(), "rule");
    assert!(h.get("x-hookbox-rule-id").is_some());
    assert_eq!(h.get("x-hookbox-plane").unwrap(), "mock");
}

#[tokio::test]
async fn unknown_404_and_gone_410_not_traced() {
    let (app, secret) = app().await;
    // Unknown token -> 404 unknown_endpoint.
    let (s, body, h) = call(&app, "GET", "ghosttoken.mock.local", "/x", None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], json!("unknown_endpoint"));
    assert_eq!(h.get("x-hookbox-endpoint").unwrap(), "ghosttoken");

    // Tombstoned token -> 410 endpoint_gone.
    let token = new_endpoint(&app, &secret).await;
    call(
        &app,
        "DELETE",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        None,
    )
    .await;
    let host = format!("{token}.mock.local");
    let (s, body, _) = call(&app, "GET", &host, "/x", None, None).await;
    assert_eq!(s, StatusCode::GONE);
    assert_eq!(body["error"], json!("endpoint_gone"));
}

#[tokio::test]
async fn default_modes_404_and_echo() {
    let (app, secret) = app().await;
    // mock_404 default.
    let token = new_endpoint(&app, &secret).await;
    let host = format!("{token}.mock.local");
    let (s, body, h) = call(&app, "GET", &host, "/nothing", None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], json!("no_match"));
    assert_eq!(h.get("x-hookbox-served-by").unwrap(), "default");

    // echo mode.
    call(
        &app,
        "PATCH",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        Some(json!({"default_mode":"echo"})),
    )
    .await;
    let (s, body, _) = call(
        &app,
        "POST",
        &host,
        "/echo/me?q=1",
        None,
        Some(json!({"a":1})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["method"], json!("POST"));
    assert_eq!(body["path"], json!("/echo/me"));
    assert_eq!(body["query"]["q"], json!("1"));
}

// --- §5.5 Auto-CRUD -----------------------------------------------------------

#[tokio::test]
async fn auto_crud_lifecycle() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    call(
        &app,
        "PATCH",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        Some(json!({"auto_crud": true})),
    )
    .await;
    let host = format!("{token}.mock.local");

    // POST -> 201 with server uuid id.
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
    assert_eq!(body["title"], json!("Dune"));

    // GET list.
    let (s, body, _) = call(&app, "GET", &host, "/books", None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 1);

    // GET by id, PUT, PATCH, DELETE.
    let (s, _b, _) = call(&app, "GET", &host, &format!("/books/{id}"), None, None).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _b, _) = call(
        &app,
        "PUT",
        &host,
        &format!("/books/{id}"),
        None,
        Some(json!({"title":"Foundation"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _b, _) = call(
        &app,
        "PATCH",
        &host,
        &format!("/books/{id}"),
        None,
        Some(json!({"year":1951})),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, _b, _) = call(&app, "DELETE", &host, &format!("/books/{id}"), None, None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _b, _) = call(&app, "GET", &host, &format!("/books/{id}"), None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Non-object write body -> 400.
    let (s, body, _) = call(&app, "POST", &host, "/books", None, Some(json!([1, 2, 3]))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], json!("bad_request"));
}

// --- §5.5 Auto-CORS -----------------------------------------------------------

#[tokio::test]
async fn cors_preflight_and_response_headers() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await; // cors_enabled defaults true
    let host = format!("{token}.mock.local");

    // OPTIONS preflight -> 204 with reflected origin, never Allow-Credentials.
    let req = Request::builder()
        .method("OPTIONS")
        .uri("/anything")
        .header("host", &host)
        .header("origin", "https://app.example.com")
        .header("access-control-request-headers", "x-custom")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let h = resp.headers();
    assert_eq!(
        h.get("access-control-allow-origin").unwrap(),
        "https://app.example.com"
    );
    assert_eq!(h.get("access-control-allow-headers").unwrap(), "x-custom");
    assert!(h.get("access-control-allow-credentials").is_none());
    assert_eq!(h.get("x-hookbox-served-by").unwrap(), "cors");

    // Non-preflight P1 response carries CORS too.
    let req = Request::builder()
        .method("GET")
        .uri("/x")
        .header("host", &host)
        .header("origin", "https://x.io")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.headers().get("access-control-allow-origin").unwrap(),
        "https://x.io"
    );

    // P2 (/api) emits NO wildcard CORS.
    let (_s, _b, h) = call(
        &app,
        "GET",
        "app.local",
        "/api/endpoints",
        Some(&secret),
        None,
    )
    .await;
    assert!(h.get("access-control-allow-origin").is_none());
}

// --- §5.5 rate limit + chaos --------------------------------------------------

#[tokio::test]
async fn rate_limit_429_with_headers() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    call(
        &app,
        "PATCH",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        Some(json!({"rate_limit_per_min": 2})),
    )
    .await;
    let host = format!("{token}.mock.local");
    // first 2 ok (default 404), 3rd -> 429.
    call(&app, "GET", &host, "/x", None, None).await;
    call(&app, "GET", &host, "/x", None, None).await;
    let (s, body, h) = call(&app, "GET", &host, "/x", None, None).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"], json!("rate_limited"));
    assert!(h.get("retry-after").is_some());
    assert_eq!(h.get("x-ratelimit-limit").unwrap(), "2");
    assert_eq!(h.get("x-hookbox-served-by").unwrap(), "ratelimit");
}

#[tokio::test]
async fn chaos_100pct_always_fires_5xx() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    call(
        &app,
        "PATCH",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        Some(json!({"chaos_pct": 100})),
    )
    .await;
    let host = format!("{token}.mock.local");
    let (s, body, h) = call(&app, "GET", &host, "/x", None, None).await;
    assert!(s.is_server_error());
    assert_eq!(body["error"], json!("chaos"));
    assert_eq!(h.get("x-hookbox-served-by").unwrap(), "chaos");
}

// --- §5.7 templating sandbox --------------------------------------------------

#[tokio::test]
async fn templating_ssti_probes_are_literal() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    add_rule(&app, &secret, &token, json!({
        "match": {"method":"GET","path":"/t"},
        "response": {"content_type":"text/plain", "body_template": "{{7*7}}|{{config}}|{{request.method}}"}
    })).await;
    let host = format!("{token}.mock.local");
    let req = Request::builder()
        .method("GET")
        .uri("/t")
        .header("host", &host)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(text, "{{7*7}}|{{config}}|GET"); // SSTI probes verbatim; real tag rendered
}

// --- state writes + gating ----------------------------------------------------

#[tokio::test]
async fn state_write_then_gated_rule() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    // Rule A: POST /login writes state logged_in=1 and echoes it.
    add_rule(
        &app,
        &secret,
        &token,
        json!({
            "priority": 10,
            "match": {"method":"POST","path":"/login"},
            "state_writes": [{"key":"logged_in","value":"1"}],
            "response": {"content_type":"text/plain","body_template":"{{state.logged_in}}"}
        }),
    )
    .await;
    // Rule B: GET /secret only when logged_in == 1.
    add_rule(&app, &secret, &token, json!({
        "priority": 10,
        "match": {"method":"GET","path":"/secret","state_requirements":[{"key":"logged_in","op":"eq","value":"1"}]},
        "response": {"status_code": 200, "content_type":"text/plain","body_template":"ok"}
    })).await;
    let host = format!("{token}.mock.local");

    // Before login: gated rule fails closed -> default 404.
    let (s, _b, _) = call(&app, "GET", &host, "/secret", None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // Login writes state; the same response sees the just-written value.
    let req = Request::builder()
        .method("POST")
        .uri("/login")
        .header("host", &host)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(String::from_utf8_lossy(&bytes), "1");

    // Now the gated rule matches.
    let (s, _b, h) = call(&app, "GET", &host, "/secret", None, None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h.get("x-hookbox-served-by").unwrap(), "rule");

    // The management state peek reflects it.
    let (s, body, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/endpoints/{token}/state"),
        Some(&secret),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["state"]["logged_in"], json!("1"));
}

// --- trace persistence + redaction --------------------------------------------

#[tokio::test]
async fn served_requests_are_traced_with_redaction() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    add_rule(
        &app,
        &secret,
        &token,
        json!({"match":{"method":"GET","path":"/p"},"response":{"status_code":200}}),
    )
    .await;
    let host = format!("{token}.mock.local");
    // include an Authorization header that must be redacted in the stored trace.
    let req = Request::builder()
        .method("GET")
        .uri("/p")
        .header("host", &host)
        .header("authorization", "Bearer leak")
        .body(Body::empty())
        .unwrap();
    let _ = app.clone().oneshot(req).await.unwrap();

    // Give the spawned trace task a moment.
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let (s, list, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/endpoints/{token}/requests"),
        Some(&secret),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let arr = list.as_array().unwrap();
    assert!(!arr.is_empty(), "served request should be traced");
    let id = arr[0]["id"].as_i64().unwrap();
    let (s, detail, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/requests/{id}"),
        Some(&secret),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    // owner cap redacted before persist.
    assert_eq!(
        detail["request_headers"]["authorization"],
        json!("<redacted>")
    );
    assert_eq!(detail["served_by"], json!("rule"));
}

// === F4 — share links (BE-1: §5.1 owner routes + §5.2 public resolver) ========

async fn mint_share(
    app: &axum::Router,
    secret: &str,
    token: &str,
    label: Option<&str>,
) -> (StatusCode, Value, axum::http::HeaderMap) {
    let body = match label {
        Some(l) => json!({ "label": l }),
        None => json!({}),
    };
    call(
        app,
        "POST",
        "app.local",
        &format!("/api/endpoints/{token}/shares"),
        Some(secret),
        Some(body),
    )
    .await
}

async fn list_shares_of(
    app: &axum::Router,
    secret: &str,
    token: &str,
) -> (StatusCode, Value, axum::http::HeaderMap) {
    call(
        app,
        "GET",
        "app.local",
        &format!("/api/endpoints/{token}/shares"),
        Some(secret),
        None,
    )
    .await
}

async fn revoke_share(
    app: &axum::Router,
    secret: &str,
    token: &str,
    id: i64,
) -> (StatusCode, Value, axum::http::HeaderMap) {
    call(
        app,
        "DELETE",
        "app.local",
        &format!("/api/endpoints/{token}/shares/{id}"),
        Some(secret),
        None,
    )
    .await
}

async fn public_list(
    app: &axum::Router,
    code: &str,
    query: &str,
) -> (StatusCode, Value, axum::http::HeaderMap) {
    let path = if query.is_empty() {
        format!("/api/share/{code}/requests")
    } else {
        format!("/api/share/{code}/requests?{query}")
    };
    call(app, "GET", "app.local", &path, None, None).await
}

async fn public_detail(
    app: &axum::Router,
    code: &str,
    id: i64,
) -> (StatusCode, Value, axum::http::HeaderMap) {
    call(
        app,
        "GET",
        "app.local",
        &format!("/api/share/{code}/requests/{id}"),
        None,
        None,
    )
    .await
}

#[tokio::test]
async fn share_mint_list_revoke_happy_path() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;

    let (s, body, h) = mint_share(&app, &secret, &token, Some("Acme vendor")).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(h.get("cache-control").unwrap(), "no-store");
    let id = body["id"].as_i64().unwrap();
    let code = body["code"].as_str().unwrap().to_string();
    assert!(body["url"]
        .as_str()
        .unwrap()
        .ends_with(&format!("/s/{code}")));
    assert_eq!(body["label"], json!("Acme vendor"));
    assert!(body["last_used_at"].is_null());
    assert!(code.len() >= 32);

    // List: the id shows up, but never the code or url (AC-25).
    let (s, list, _) = list_shares_of(&app, &secret, &token).await;
    assert_eq!(s, StatusCode::OK);
    let arr = list.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], json!(id));
    assert_eq!(arr[0]["label"], json!("Acme vendor"));
    assert!(arr[0].as_object().unwrap().get("code").is_none());
    assert!(arr[0].as_object().unwrap().get("url").is_none());

    // The freshly minted code resolves publicly.
    let (s, feed, _) = public_list(&app, &code, "").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(feed["requests"], json!([]));

    // Revoke -> 204, then the list is empty and the code 404s (AC-37: takes
    // effect on the very next request — share_links is never cached).
    let (s, body, _) = revoke_share(&app, &secret, &token, id).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert!(body.is_null());
    let (s, list, _) = list_shares_of(&app, &secret, &token).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 0);
    let (s, _b, _) = public_list(&app, &code, "").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn share_owner_routes_401_missing_malformed_unknown_bearer() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let id = body["id"].as_i64().unwrap();

    // Missing Authorization entirely, across all three owner routes.
    for (method, path) in [
        ("POST", format!("/api/endpoints/{token}/shares")),
        ("GET", format!("/api/endpoints/{token}/shares")),
        ("DELETE", format!("/api/endpoints/{token}/shares/{id}")),
    ] {
        let req = Request::builder()
            .method(method)
            .uri(&path)
            .header("host", "app.local")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "{method} {path} without Authorization"
        );
        assert_eq!(resp.headers().get("www-authenticate").unwrap(), "Bearer");
    }

    // Malformed scheme (Basic, not Bearer).
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/endpoints/{token}/shares"))
        .header("host", "app.local")
        .header("authorization", "Basic xyz")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // A syntactically valid but unknown secret, across all three routes.
    let (s, _b, _) = mint_share(&app, "not-a-real-secret", &token, None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _b, _) = list_shares_of(&app, "not-a-real-secret", &token).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    let (s, _b, _) = revoke_share(&app, "not-a-real-secret", &token, id).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn share_owner_routes_404_not_403_for_foreign_token() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let id = body["id"].as_i64().unwrap();

    // A second, unrelated owner.
    let (_s, body2, _) = call(
        &app,
        "POST",
        "app.local",
        "/api/session",
        None,
        Some(json!({"email":"stranger@e.com"})),
    )
    .await;
    let stranger_secret = body2["owner_secret"].as_str().unwrap().to_string();

    let (s, body, _) = mint_share(&app, &stranger_secret, &token, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], json!("not_found"));
    let (s, _b, _) = list_shares_of(&app, &stranger_secret, &token).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _b, _) = revoke_share(&app, &stranger_secret, &token, id).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn share_active_cap_422() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    // Default cap (SHARE_MAX_PER_ENDPOINT) is 10.
    for _ in 0..10 {
        let (s, _b, _) = mint_share(&app, &secret, &token, None).await;
        assert_eq!(s, StatusCode::CREATED);
    }
    let (s, body, _) = mint_share(&app, &secret, &token, None).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], json!("validation_error"));
}

#[tokio::test]
async fn share_label_422_over_80_chars_and_blank_trims_to_null() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    let long = "x".repeat(81);
    let (s, body, _) = mint_share(&app, &secret, &token, Some(&long)).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], json!("validation_error"));

    // Exactly 80 is fine; a label that trims to empty stores as NULL.
    let ok = "y".repeat(80);
    let (s, _b, _) = mint_share(&app, &secret, &token, Some(&ok)).await;
    assert_eq!(s, StatusCode::CREATED);
    let (s, body, _) = mint_share(&app, &secret, &token, Some("   ")).await;
    assert_eq!(s, StatusCode::CREATED);
    assert!(body["label"].is_null());
}

#[tokio::test]
async fn share_mint_on_tombstoned_endpoint_404() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    call(
        &app,
        "DELETE",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        None,
    )
    .await;
    let (s, body, _) = mint_share(&app, &secret, &token, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], json!("not_found"));
}

#[tokio::test]
async fn share_tombstone_revokes_all_and_belt_and_braces_gone_at_check() {
    std::env::set_var("MOCK_DOMAIN", "mock.local");
    // The session anti-enumeration limiter (routes::api::session_rate_limited)
    // is a single process-wide static bucket keyed by client IP, and every
    // test in this binary that calls `Config::from_env()` shares it (there is
    // no real ConnectInfo in the `oneshot` harness, so ALL of them collapse
    // onto the "unknown" bucket). With ~35 tests in this file each minting at
    // least one session, the default 30/min default would intermittently 429
    // unrelated tests. Raise it here, in every place this file builds a
    // `Config`, so the shared bucket never binds — this does not change any
    // production default (the env var is scoped to this test binary process).
    std::env::set_var("SESSION_RATE_LIMIT_PER_MIN", "1000000");
    std::env::set_var("APP_HOST", "app.local");
    let pool = db::pool(":memory:").await.unwrap();
    db::migrate(&pool).await.unwrap();
    let state = AppState::new(pool.clone(), Config::from_env());
    let app = build_app(state);
    let (_s, sbody, _) = call(
        &app,
        "POST",
        "app.local",
        "/api/session",
        None,
        Some(json!({"email":"tomb@e.com"})),
    )
    .await;
    let secret = sbody["owner_secret"].as_str().unwrap().to_string();
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    call(
        &app,
        "DELETE",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        None,
    )
    .await;

    // The share_links revoke ran as part of the tombstone -> already 404.
    let (s, _b, _) = public_list(&app, &code, "").await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    // AC-S9 belt-and-braces: simulate the revoke statement having failed by
    // directly clearing revoked_at back to NULL, as if the UPDATE never took
    // effect. The resolver's OWN `e.gone_at IS NULL` check in the same joined
    // statement still 404s regardless.
    sqlx::query("UPDATE share_links SET revoked_at = NULL WHERE token = ?")
        .bind(&token)
        .execute(&pool)
        .await
        .unwrap();
    let (s, _b, _) = public_list(&app, &code, "").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn share_404_identity_unknown_revoked_tombstoned_byte_identical_incl_head() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;

    // Unknown code: correctly shaped, never minted.
    let unknown_code = ids::gen_share_code(24);

    // Revoked code.
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let revoked_id = body["id"].as_i64().unwrap();
    let revoked_code = body["code"].as_str().unwrap().to_string();
    revoke_share(&app, &secret, &token, revoked_id).await;

    // Tombstoned-endpoint code.
    let token2 = new_endpoint(&app, &secret).await;
    let (_s, body2, _) = mint_share(&app, &secret, &token2, None).await;
    let tombstoned_code = body2["code"].as_str().unwrap().to_string();
    call(
        &app,
        "DELETE",
        "app.local",
        &format!("/api/endpoints/{token2}"),
        Some(&secret),
        None,
    )
    .await;

    let mut bodies = Vec::new();
    for code in [&unknown_code, &revoked_code, &tombstoned_code] {
        let (s, b, h) = public_list(&app, code, "").await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        assert_eq!(h.get("cache-control").unwrap(), "no-store");
        bodies.push(b);
    }
    assert_eq!(bodies[0], bodies[1]);
    assert_eq!(bodies[1], bodies[2]);

    // HEAD is auto-implemented and rate-limited too; same status + headers.
    let req = Request::builder()
        .method("HEAD")
        .uri(format!("/api/share/{unknown_code}/requests"))
        .header("host", "app.local")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(resp.headers().get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn share_detail_cross_endpoint_request_id_404() {
    let (app, secret) = app().await;
    let token_a = new_endpoint(&app, &secret).await;
    add_rule(
        &app,
        &secret,
        &token_a,
        json!({"match":{"method":"GET","path":"/p"},"response":{"status_code":200}}),
    )
    .await;
    let host_a = format!("{token_a}.mock.local");
    call(&app, "GET", &host_a, "/p", None, None).await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let (_s, list_a, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/endpoints/{token_a}/requests"),
        Some(&secret),
        None,
    )
    .await;
    let request_id_a = list_a.as_array().unwrap()[0]["id"].as_i64().unwrap();

    let token_b = new_endpoint(&app, &secret).await;
    let (_s, body_b, _) = mint_share(&app, &secret, &token_b, None).await;
    let code_b = body_b["code"].as_str().unwrap().to_string();

    // request_id_a belongs to token_a; code_b's share is scoped to token_b —
    // the `AND token = ?` inside the resolved statement is the whole of AC-35.
    let (s, _b, _) = public_detail(&app, &code_b, request_id_a).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn share_limit_validation_precedes_code_resolution() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let valid_code = body["code"].as_str().unwrap().to_string();
    let invalid_code = ids::gen_share_code(24); // correctly shaped, but unknown

    // `?limit=999` -> 422 for BOTH a valid and an invalid code (AC-101): if
    // this were checked after code resolution, the two would differ (422 vs
    // 404) and that difference would be a live/dead-code oracle.
    for code in [&valid_code, &invalid_code] {
        let (s, body, _) = public_list(&app, code, "limit=999").await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"], json!("validation_error"));
    }
    let (s, body, _) = public_list(&app, &valid_code, "offset=-1").await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], json!("validation_error"));
}

#[tokio::test]
async fn share_rate_limit_429_per_ip_with_retry_after() {
    std::env::set_var("MOCK_DOMAIN", "mock.local");
    // The session anti-enumeration limiter (routes::api::session_rate_limited)
    // is a single process-wide static bucket keyed by client IP, and every
    // test in this binary that calls `Config::from_env()` shares it (there is
    // no real ConnectInfo in the `oneshot` harness, so ALL of them collapse
    // onto the "unknown" bucket). With ~35 tests in this file each minting at
    // least one session, the default 30/min default would intermittently 429
    // unrelated tests. Raise it here, in every place this file builds a
    // `Config`, so the shared bucket never binds — this does not change any
    // production default (the env var is scoped to this test binary process).
    std::env::set_var("SESSION_RATE_LIMIT_PER_MIN", "1000000");
    std::env::set_var("APP_HOST", "app.local");
    let pool = db::pool(":memory:").await.unwrap();
    db::migrate(&pool).await.unwrap();
    let mut cfg = Config::from_env();
    cfg.share_rate_limit_per_min = 2;
    let state = AppState::new(pool, cfg);
    let app = build_app(state);
    let (_s, sbody, _) = call(
        &app,
        "POST",
        "app.local",
        "/api/session",
        None,
        Some(json!({"email":"rl@e.com"})),
    )
    .await;
    let secret = sbody["owner_secret"].as_str().unwrap().to_string();
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    let (s1, _b, _) = public_list(&app, &code, "").await;
    assert_eq!(s1, StatusCode::OK);
    let (s2, _b, _) = public_list(&app, &code, "").await;
    assert_eq!(s2, StatusCode::OK);
    let (s3, body3, h3) = public_list(&app, &code, "").await;
    assert_eq!(s3, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body3["error"], json!("rate_limited"));
    assert!(h3.get("retry-after").is_some());
    assert_eq!(h3.get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn share_rate_limit_429_global_ceiling() {
    std::env::set_var("MOCK_DOMAIN", "mock.local");
    // The session anti-enumeration limiter (routes::api::session_rate_limited)
    // is a single process-wide static bucket keyed by client IP, and every
    // test in this binary that calls `Config::from_env()` shares it (there is
    // no real ConnectInfo in the `oneshot` harness, so ALL of them collapse
    // onto the "unknown" bucket). With ~35 tests in this file each minting at
    // least one session, the default 30/min default would intermittently 429
    // unrelated tests. Raise it here, in every place this file builds a
    // `Config`, so the shared bucket never binds — this does not change any
    // production default (the env var is scoped to this test binary process).
    std::env::set_var("SESSION_RATE_LIMIT_PER_MIN", "1000000");
    std::env::set_var("APP_HOST", "app.local");
    let pool = db::pool(":memory:").await.unwrap();
    db::migrate(&pool).await.unwrap();
    let mut cfg = Config::from_env();
    cfg.share_rate_limit_per_min = 1_000; // effectively unlimited per source
    cfg.share_rate_limit_global_per_min = 2; // tiny instance-wide ceiling
    let state = AppState::new(pool, cfg);
    let app = build_app(state);
    let (_s, sbody, _) = call(
        &app,
        "POST",
        "app.local",
        "/api/session",
        None,
        Some(json!({"email":"glob@e.com"})),
    )
    .await;
    let secret = sbody["owner_secret"].as_str().unwrap().to_string();
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    for i in 0..2 {
        let (s, _b, _) = public_list(&app, &code, "").await;
        assert_eq!(s, StatusCode::OK, "request {i} should pass");
    }
    let (s, _b, h) = public_list(&app, &code, "").await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(h.get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn share_public_routes_503_carries_no_store() {
    std::env::set_var("MOCK_DOMAIN", "mock.local");
    // The session anti-enumeration limiter (routes::api::session_rate_limited)
    // is a single process-wide static bucket keyed by client IP, and every
    // test in this binary that calls `Config::from_env()` shares it (there is
    // no real ConnectInfo in the `oneshot` harness, so ALL of them collapse
    // onto the "unknown" bucket). With ~35 tests in this file each minting at
    // least one session, the default 30/min default would intermittently 429
    // unrelated tests. Raise it here, in every place this file builds a
    // `Config`, so the shared bucket never binds — this does not change any
    // production default (the env var is scoped to this test binary process).
    std::env::set_var("SESSION_RATE_LIMIT_PER_MIN", "1000000");
    std::env::set_var("APP_HOST", "app.local");
    let pool = db::pool(":memory:").await.unwrap();
    db::migrate(&pool).await.unwrap();
    let owner_id = "owner503aaaaaaaa";
    sqlx::query("INSERT INTO owners (owner_id, email, secret_hash) VALUES (?, ?, ?)")
        .bind(owner_id)
        .bind("x@y.com")
        .bind("hash")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO endpoints (token, owner_id) VALUES ('tok5030001', ?)")
        .bind(owner_id)
        .execute(&pool)
        .await
        .unwrap();
    let code = ids::gen_share_code(24);
    let code_hash = ids::hash_secret(&code);
    sqlx::query("INSERT INTO share_links (code_hash, token) VALUES (?, 'tok5030001')")
        .bind(&code_hash)
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    let cfg = Config::from_env();
    let state = AppState::new(pool, cfg);
    let app = build_app(state);

    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/share/{code}/requests"))
        .header("host", "app.local")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(resp.headers().get("cache-control").unwrap(), "no-store");
}

#[tokio::test]
async fn share_public_routes_405_for_other_verbs_and_tables_unchanged() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    for method in ["POST", "PATCH", "PUT", "DELETE"] {
        let req = Request::builder()
            .method(method)
            .uri(format!("/api/share/{code}/requests"))
            .header("host", "app.local")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "{method} should 405"
        );
    }
    // The share itself is still there (untouched — no accidental mutation).
    let (s, list, _) = list_shares_of(&app, &secret, &token).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn share_public_detail_key_allowlist_and_no_token_substring() {
    let (app, secret) = app().await;
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

    let (_s, list, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/endpoints/{token}/requests"),
        Some(&secret),
        None,
    )
    .await;
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

    // AC-S2: token absence is asserted AFTER the filter, scoped to
    // server-generated fields only (`response_headers` + the
    // summary/identity fields) — never across `request_headers`/`path`/
    // `query_params`/`request_body`, which are caller-supplied and, by
    // design, land in the projection verbatim (e.g. the mock host's `Host`
    // request header legitimately embeds the token). An unscoped
    // "token is not a substring anywhere" assertion is a flaky test the PRD
    // explicitly calls out and rejects for exactly this reason.
    let response_headers_raw = serde_json::to_string(&detail["response_headers"]).unwrap();
    assert!(
        !response_headers_raw.contains(&token),
        "the filtered response_headers must never contain the endpoint token"
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
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    add_rule(&app, &secret, &token, json!({
        "match": {"method":"GET","path":"/p"},
        "response": {"status_code": 200, "headers": {"set-cookie": "sid=abc"}, "content_type":"text/plain","body_template":"ok"}
    })).await;
    let host = format!("{token}.mock.local");
    call(&app, "GET", &host, "/p", None, None).await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let (_s, list, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/endpoints/{token}/requests"),
        Some(&secret),
        None,
    )
    .await;
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
    let (app, secret) = app().await;
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

    let (_s, list, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/endpoints/{token}/requests"),
        Some(&secret),
        None,
    )
    .await;
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

#[tokio::test]
async fn share_last_used_at_written_off_path_and_coalesced() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();

    let (_s, list, _) = list_shares_of(&app, &secret, &token).await;
    assert!(list.as_array().unwrap()[0]["last_used_at"].is_null());

    public_list(&app, &code, "").await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let (_s, list, _) = list_shares_of(&app, &secret, &token).await;
    let first_touch = list.as_array().unwrap()[0]["last_used_at"].clone();
    assert!(!first_touch.is_null());

    // A second poll within 60s does not move it again (coalesced, AC-S10).
    public_list(&app, &code, "").await;
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let (_s, list, _) = list_shares_of(&app, &secret, &token).await;
    assert_eq!(list.as_array().unwrap()[0]["last_used_at"], first_touch);
}

#[tokio::test]
async fn share_code_never_appears_in_error_detail() {
    let (app, _secret) = app().await;
    let code = ids::gen_share_code(24);
    let (s, body, _) = public_list(&app, &code, "").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let detail = body["detail"].as_str().unwrap();
    assert!(!detail.contains(&code));
}

#[tokio::test]
async fn share_url_built_from_public_base_url_never_mock_host() {
    std::env::set_var("MOCK_DOMAIN", "mock.local");
    // The session anti-enumeration limiter (routes::api::session_rate_limited)
    // is a single process-wide static bucket keyed by client IP, and every
    // test in this binary that calls `Config::from_env()` shares it (there is
    // no real ConnectInfo in the `oneshot` harness, so ALL of them collapse
    // onto the "unknown" bucket). With ~35 tests in this file each minting at
    // least one session, the default 30/min default would intermittently 429
    // unrelated tests. Raise it here, in every place this file builds a
    // `Config`, so the shared bucket never binds — this does not change any
    // production default (the env var is scoped to this test binary process).
    std::env::set_var("SESSION_RATE_LIMIT_PER_MIN", "1000000");
    std::env::set_var("APP_HOST", "app.local");
    let pool = db::pool(":memory:").await.unwrap();
    db::migrate(&pool).await.unwrap();
    let mut cfg = Config::from_env();
    cfg.public_base_url = "https://hookbox.example.com".to_string();
    let state = AppState::new(pool, cfg);
    let app = build_app(state);
    let (_s, sbody, _) = call(
        &app,
        "POST",
        "app.local",
        "/api/session",
        None,
        Some(json!({"email":"url@e.com"})),
    )
    .await;
    let secret = sbody["owner_secret"].as_str().unwrap().to_string();
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let code = body["code"].as_str().unwrap().to_string();
    assert_eq!(
        body["url"],
        json!(format!("https://hookbox.example.com/s/{code}"))
    );
    assert!(!body["url"].as_str().unwrap().contains("mock.local"));
}

#[tokio::test]
async fn share_revoke_without_auth_401_and_nothing_revoked() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    let (_s, body, _) = mint_share(&app, &secret, &token, None).await;
    let id = body["id"].as_i64().unwrap();

    let (s, _b, _) = call(
        &app,
        "DELETE",
        "app.local",
        &format!("/api/endpoints/{token}/shares/{id}"),
        None,
        None,
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);

    let (_s, list, _) = list_shares_of(&app, &secret, &token).await;
    assert_eq!(list.as_array().unwrap().len(), 1); // still active, nothing revoked
}

// === F7 — response-body capture (BE-2) =========================================

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
    std::env::set_var("MOCK_DOMAIN", "mock.local");
    std::env::set_var("SESSION_RATE_LIMIT_PER_MIN", "1000000");
    std::env::set_var("APP_HOST", "app.local");
    let pool = db::pool(":memory:").await.unwrap();
    db::migrate(&pool).await.unwrap();
    let mut cfg = Config::from_env();
    cfg.mitm_allow_private = true;
    let state = AppState::new(pool, cfg);
    let app = build_app(state);
    let (_s, sbody, _) = call(
        &app,
        "POST",
        "app.local",
        "/api/session",
        None,
        Some(json!({"email":"mitm@e.com"})),
    )
    .await;
    let secret = sbody["owner_secret"].as_str().unwrap().to_string();
    (app, secret)
}

/// Build a fresh app with a raised `template_max_size` — F7's persist cap
/// (`max_body_bytes`, default 256_000) and the RULE VALIDATION cap
/// (`template_max_size`, ALSO 256_000 by default) happen to share a default
/// value, so a `body_template` big enough to exercise truncation on the
/// persist side would be rejected by rule creation itself unless this is
/// raised. `max_body_bytes` (the value under test) is left at its default.
async fn app_with_larger_template_cap() -> (axum::Router, String) {
    std::env::set_var("MOCK_DOMAIN", "mock.local");
    std::env::set_var("SESSION_RATE_LIMIT_PER_MIN", "1000000");
    std::env::set_var("APP_HOST", "app.local");
    let pool = db::pool(":memory:").await.unwrap();
    db::migrate(&pool).await.unwrap();
    let mut cfg = Config::from_env();
    cfg.template_max_size = 400_000;
    let state = AppState::new(pool, cfg);
    let app = build_app(state);
    let (_s, sbody, _) = call(
        &app,
        "POST",
        "app.local",
        "/api/session",
        None,
        Some(json!({"email":"trunc@e.com"})),
    )
    .await;
    let secret = sbody["owner_secret"].as_str().unwrap().to_string();
    (app, secret)
}

async fn latest_request_detail(app: &axum::Router, secret: &str, token: &str) -> Value {
    let (_s, list, _) = call(
        app,
        "GET",
        "app.local",
        &format!("/api/endpoints/{token}/requests"),
        Some(secret),
        None,
    )
    .await;
    let request_id = list.as_array().unwrap()[0]["id"].as_i64().unwrap();
    let (_s, detail, _) = call(
        app,
        "GET",
        "app.local",
        &format!("/api/requests/{request_id}"),
        Some(secret),
        None,
    )
    .await;
    detail
}

#[tokio::test]
async fn f7_rule_response_body_captured_exactly() {
    let (app, secret) = app().await;
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
    let (app, secret) = app().await;
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
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    call(
        &app,
        "PATCH",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        Some(json!({"rate_limit_per_min": 1})),
    )
    .await;
    let host = format!("{token}.mock.local");
    call(&app, "GET", &host, "/x", None, None).await; // consumes the one token
    let (s, _b, _) = call(&app, "GET", &host, "/x", None, None).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;

    let detail = latest_request_detail(&app, &secret, &token).await;
    assert_eq!(detail["served_by"], json!("ratelimit"));
    let stored = detail["response_body"].as_str().unwrap();
    assert!(stored.contains("rate_limited"));
}

#[tokio::test]
async fn f7_chaos_status_captured_and_chaos_drop_is_null() {
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    call(
        &app,
        "PATCH",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        Some(json!({"chaos_pct": 100, "chaos_mode": "error"})),
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
    call(
        &app,
        "PATCH",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        Some(json!({"chaos_mode": "dropout"})),
    )
    .await;
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
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    call(
        &app,
        "PATCH",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        Some(json!({"auto_crud": true})),
    )
    .await;
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
    let (app, secret) = app().await;
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
    let (app, secret) = app().await;
    let token = new_endpoint(&app, &secret).await;
    call(
        &app,
        "PATCH",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        Some(json!({"default_mode":"echo"})),
    )
    .await;
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
    call(
        &app,
        "PATCH",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        Some(json!({"target_url": upstream})),
    )
    .await;

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
    call(
        &app,
        "PATCH",
        "app.local",
        &format!("/api/endpoints/{token}"),
        Some(&secret),
        Some(json!({"target_url": upstream})),
    )
    .await;

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

    let (_s, list, _) = call(
        &app,
        "GET",
        "app.local",
        &format!("/api/endpoints/{token}/requests"),
        Some(&secret),
        None,
    )
    .await;
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
    let (app, secret) = app().await;
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
    std::env::set_var("MOCK_DOMAIN", "mock.local");
    std::env::set_var("SESSION_RATE_LIMIT_PER_MIN", "1000000");
    std::env::set_var("APP_HOST", "app.local");
    let pool = db::pool(":memory:").await.unwrap();
    db::migrate(&pool).await.unwrap();
    let state = AppState::new(pool, Config::from_env());
    let feed_hub = state.feed_hub.clone();
    let app = build_app(state);
    let (_s, sbody, _) = call(
        &app,
        "POST",
        "app.local",
        "/api/session",
        None,
        Some(json!({"email":"feed@e.com"})),
    )
    .await;
    let secret = sbody["owner_secret"].as_str().unwrap().to_string();
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
    let (app, secret) = app().await;
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
