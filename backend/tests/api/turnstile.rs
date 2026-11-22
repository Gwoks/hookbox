//! Cloudflare Turnstile on the first anonymous mint (`POST /api/session`).
//!
//! A tiny in-process siteverify stub on `127.0.0.1:0` answers with canned
//! JSON keyed on the submitted token and counts its calls, so every outcome
//! (pass, `success:false`, wrong action / hostname, 5xx, unreachable,
//! oversized token) is exercised without touching Cloudflare.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{ConnectInfo, State};
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use hookbox::config::Secret;

use crate::common::{
    app_with_config, call_with, cookie_header, mint_session, session_cookie_key, Reply, APP_HOST,
    CSRF,
};

const STUB_SECRET: &str = "stub-secret";

/// What the stub saw: total calls and the last `remoteip` it was sent.
#[derive(Clone, Default)]
struct StubLog {
    calls: Arc<AtomicUsize>,
    last_remoteip: Arc<Mutex<Option<String>>>,
}

impl StubLog {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

/// Canned siteverify answers, keyed on the token (`response`).
fn canned(token: &str) -> Value {
    let pass = |action: &str, hostname: &str| json!({"success": true, "action": action, "hostname": hostname, "error-codes": []});
    match token {
        "good" => pass("session_mint", APP_HOST),
        "good-upper" => pass("session_mint", "APP.LOCAL"),
        "wrong-action" => pass("login", APP_HOST),
        "wrong-host" => pass("session_mint", "evil.example"),
        _ => json!({"success": false, "error-codes": ["invalid-input-response"]}),
    }
}

async fn siteverify(State(log): State<StubLog>, Json(req): Json<Value>) -> Response {
    log.calls.fetch_add(1, Ordering::SeqCst);
    *log.last_remoteip.lock().unwrap() = req["remoteip"].as_str().map(str::to_string);
    if req["secret"] != STUB_SECRET {
        return Json(json!({"success": false, "error-codes": ["invalid-input-secret"]}))
            .into_response();
    }
    match req["response"].as_str().unwrap_or_default() {
        "boom" => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        token => Json(canned(token)).into_response(),
    }
}

/// Start the stub; returns its siteverify URL and call log.
async fn start_stub() -> (String, StubLog) {
    let log = StubLog::default();
    let app = Router::new()
        .route("/siteverify", post(siteverify))
        .with_state(log.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}/siteverify"), log)
}

/// The app with Turnstile enabled against `verify_url` (hostnames default
/// to `[APP_HOST]`).
async fn app_verifying_at(verify_url: String) -> Router {
    let (app, _) = app_with_config(|cfg| {
        cfg.turnstile_secret = Some(Secret::new(STUB_SECRET));
        cfg.turnstile_verify_url = verify_url;
    })
    .await;
    app
}

async fn app_with_stub() -> (Router, StubLog) {
    let (url, log) = start_stub().await;
    (app_verifying_at(url).await, log)
}

async fn mint_with_token(app: &Router, token: &str) -> Reply {
    let body = json!({ "turnstile_token": token });
    call_with(app, "POST", APP_HOST, "/api/session", &[CSRF], Some(body)).await
}

fn assert_turnstile_failed((status, body, headers): &Reply) {
    assert_eq!(*status, StatusCode::FORBIDDEN, "{body:?}");
    assert_eq!(body["error"], "turnstile_failed");
    assert!(body["detail"].is_string());
    assert!(
        session_cookie_key(headers).is_none(),
        "no cookie on failure"
    );
    assert_eq!(headers.get("cache-control").unwrap(), "no-store");
}

/// A raw mint (verbatim body, optional TCP peer) for cases `call_with`
/// cannot express.
async fn mint_raw(app: &Router, body: &'static str, peer: Option<&str>, real_ip: &str) -> Reply {
    let mut req = Request::builder()
        .method("POST")
        .uri("/api/session")
        .header("host", APP_HOST)
        .header(CSRF.0, CSRF.1)
        .header("x-real-ip", real_ip)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    if let Some(ip) = peer {
        let addr: SocketAddr = format!("{ip}:40000").parse().unwrap();
        req.extensions_mut().insert(ConnectInfo(addr));
    }
    let resp = app.clone().oneshot(req).await.unwrap();
    let (status, headers) = (resp.status(), resp.headers().clone());
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, v, headers)
}

#[tokio::test]
async fn valid_token_mints_owner_and_sets_cookie() {
    let (app, log) = app_with_stub().await;
    let (status, body, headers) = mint_with_token(&app, "good").await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
    assert!(session_cookie_key(&headers).is_some());
    assert_eq!(body["endpoints"].as_array().unwrap().len(), 1);
    assert_eq!(log.calls(), 1);
}

#[tokio::test]
async fn hostname_allowlist_is_case_insensitive() {
    let (app, _) = app_with_stub().await;
    let (status, body, _) = mint_with_token(&app, "good-upper").await;
    assert_eq!(status, StatusCode::CREATED, "{body:?}");
}

#[tokio::test]
async fn missing_token_is_rejected_without_calling_siteverify() {
    let (app, log) = app_with_stub().await;
    assert_turnstile_failed(&mint_session(&app).await);
    let empty_object = call_with(
        &app,
        "POST",
        APP_HOST,
        "/api/session",
        &[CSRF],
        Some(json!({})),
    )
    .await;
    assert_turnstile_failed(&empty_object);
    assert_eq!(log.calls(), 0);
}

#[tokio::test]
async fn siteverify_rejections_fail_closed() {
    let (app, log) = app_with_stub().await;
    for token in ["bad", "wrong-action", "wrong-host", "boom"] {
        let reply = mint_with_token(&app, token).await;
        assert_turnstile_failed(&reply);
    }
    assert_eq!(log.calls(), 4);
}

#[tokio::test]
async fn unreachable_verify_url_fails_closed() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let app = app_verifying_at(format!("http://{addr}/siteverify")).await;
    assert_turnstile_failed(&mint_with_token(&app, "good").await);
}

#[tokio::test]
async fn oversized_token_is_rejected_without_calling_siteverify() {
    let (app, log) = app_with_stub().await;
    assert_turnstile_failed(&mint_with_token(&app, &"t".repeat(2049)).await);
    assert_eq!(log.calls(), 0);
}

#[tokio::test]
async fn cookie_holder_needs_no_token() {
    let (app, log) = app_with_stub().await;
    let (_, _, headers) = mint_with_token(&app, "good").await;
    let key = session_cookie_key(&headers).unwrap();
    let cookie = cookie_header(&key);
    let (status, body, headers) = call_with(
        &app,
        "POST",
        APP_HOST,
        "/api/session",
        &[CSRF, (cookie.0, cookie.1.as_str())],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert!(session_cookie_key(&headers).is_none());
    assert_eq!(log.calls(), 1, "only the first mint was verified");
}

#[tokio::test]
async fn malformed_json_body_is_a_client_error() {
    let (app, log) = app_with_stub().await;
    let (status, body, _) = mint_raw(&app, "{not json", None, "203.0.113.9").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
    assert_eq!(body["error"], "bad_request");
    let (status, body, _) = mint_raw(&app, r#"{"turnstile_token":5}"#, None, "x").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body:?}");
    assert_eq!(body["error"], "validation_error");
    assert_eq!(log.calls(), 0);
}

#[tokio::test]
async fn forwards_the_rate_limiter_client_ip_as_remoteip() {
    let (app, log) = app_with_stub().await;
    let body = r#"{"turnstile_token":"good"}"#;
    let (status, _, _) = mint_raw(&app, body, Some("127.0.0.1"), "203.0.113.9").await;
    assert_eq!(status, StatusCode::CREATED);
    let seen = log.last_remoteip.lock().unwrap().clone();
    assert_eq!(seen.as_deref(), Some("203.0.113.9"));
}

#[tokio::test]
async fn disabled_turnstile_mints_without_a_token() {
    let (app, _) = app_with_config(|_| {}).await;
    let (status, _, headers) = mint_session(&app).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(session_cookie_key(&headers).is_some());
}
