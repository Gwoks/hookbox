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
use hookbox::router::build_app;
use hookbox::state::AppState;

async fn app() -> (axum::Router, String) {
    std::env::set_var("MOCK_DOMAIN", "mock.local");
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
