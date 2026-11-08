//! §5.5 mock plane: resolution order, X-HookBox identifying headers, 404 vs
//! 410, Auto-CRUD, Auto-CORS, rate limit + chaos, templating sandbox, state
//! gating, and trace persistence with redaction.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::common::{add_rule, call, new_endpoint, patch_endpoint, TestApp};

#[tokio::test]
async fn mock_rule_match_renders_template_and_identifies() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    let rule_id = add_rule(&app, &secret, &token, json!({
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
    assert_eq!(
        h.get("x-hookbox-rule-id").unwrap(),
        rule_id.to_string().as_str()
    );
    assert_eq!(h.get("x-hookbox-plane").unwrap(), "mock");
}

#[tokio::test]
async fn unknown_404_and_gone_410_not_traced() {
    let TestApp { app, secret, .. } = TestApp::new().await;
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
    let TestApp { app, secret, .. } = TestApp::new().await;
    // mock_404 default.
    let token = new_endpoint(&app, &secret).await;
    let host = format!("{token}.mock.local");
    let (s, body, h) = call(&app, "GET", &host, "/nothing", None, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], json!("no_match"));
    assert_eq!(h.get("x-hookbox-served-by").unwrap(), "default");

    // echo mode.
    patch_endpoint(&app, &secret, &token, json!({"default_mode":"echo"})).await;
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

#[tokio::test]
async fn auto_crud_lifecycle() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    patch_endpoint(&app, &secret, &token, json!({"auto_crud": true})).await;
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

/// AC-122(e): a backend regression test asserting that `auto_crud = true`
/// PLUS the frozen §5.5.7 catch-all rule is served by `rule`, not `crud` — so
/// the shadowing (matched rules return before `resolve_unmatched()`, which is
/// what Auto-CRUD lives behind — `interceptor/engine.rs`) is chosen and
/// tested behaviour, not something discovered in production.
#[tokio::test]
async fn auto_crud_endpoint_with_catch_all_rule_is_served_by_rule_not_crud() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    patch_endpoint(&app, &secret, &token, json!({"auto_crud": true})).await;
    let host = format!("{token}.mock.local");

    // Before the catch-all: Auto-CRUD answers a collection path itself.
    let req = Request::builder()
        .method("GET")
        .uri("/widgets")
        .header("host", &host)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("x-hookbox-served-by").unwrap(), "crud");

    // Add the frozen §5.5.7 default catch-all rule (payload as amended by
    // AC-125's copy.md strings).
    add_rule(
        &app,
        &secret,
        &token,
        json!({
            "name": "Catch-all (default)",
            "priority": 1000,
            "enabled": true,
            "match": { "method": "ANY", "path": "/*", "headers": {}, "query": {},
                       "body_conditions": [], "state_requirements": [] },
            "response": { "status_code": 200, "headers": {}, "content_type": "application/json",
                          "body_template": "{\n  \"ok\": true,\n  \"hookbox\": \"default catch-all\",\n  \"message\": \"Edit this rule in HookBox to return your own response.\"\n}" },
            "state_writes": [], "latency_ms": null, "rate_limit_per_min": null,
            "chaos_mode": null, "webhook_action": null
        }),
    )
    .await;

    // AC-122(e): the SAME Auto-CRUD collection path is now shadowed — served
    // by the rule, not crud, and the body is the catch-all placeholder.
    let req = Request::builder()
        .method("GET")
        .uri("/widgets")
        .header("host", &host)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("x-hookbox-served-by").unwrap(), "rule");
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["hookbox"], json!("default catch-all"));

    // AC-63: an arbitrary deep path/verb is caught too, not just the
    // Auto-CRUD collection route.
    let req = Request::builder()
        .method("PATCH")
        .uri("/anything/at/all")
        .header("host", &host)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("x-hookbox-served-by").unwrap(), "rule");
}

#[tokio::test]
async fn cors_preflight_and_response_headers() {
    let TestApp { app, secret, .. } = TestApp::new().await;
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

#[tokio::test]
async fn rate_limit_429_with_headers() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    patch_endpoint(&app, &secret, &token, json!({"rate_limit_per_min": 2})).await;
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
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = new_endpoint(&app, &secret).await;
    patch_endpoint(&app, &secret, &token, json!({"chaos_pct": 100})).await;
    let host = format!("{token}.mock.local");
    let (s, body, h) = call(&app, "GET", &host, "/x", None, None).await;
    assert!(s.is_server_error());
    assert_eq!(body["error"], json!("chaos"));
    assert_eq!(h.get("x-hookbox-served-by").unwrap(), "chaos");
}

#[tokio::test]
async fn templating_ssti_probes_are_literal() {
    let TestApp { app, secret, .. } = TestApp::new().await;
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

#[tokio::test]
async fn state_write_then_gated_rule() {
    let TestApp { app, secret, .. } = TestApp::new().await;
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

#[tokio::test]
async fn served_requests_are_traced_with_redaction() {
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

// --- moved from router.rs unit tests (unique assertions only) ---

/// A mock-host `/api/...` request is P1 (the interceptor), never the
/// management API, even with a valid Bearer: the unknown token `tok123` gets
/// the interceptor's 404 `unknown_endpoint`, not an endpoint list (R6, AC-6).
#[tokio::test]
async fn mock_host_api_path_never_reaches_management() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let (s, body, h) = call(
        &app,
        "GET",
        "tok123.mock.local",
        "/api/endpoints",
        Some(&secret),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(h.get("x-hookbox-plane").unwrap(), "mock");
    assert_eq!(body["error"], json!("unknown_endpoint"));
}
