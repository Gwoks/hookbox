//! Top-level plane dispatch — PORT of `app/middleware.py::PlaneDispatchMiddleware`
//! + the route-order guard from `app/main.py`.
//!
//! A `from_fn` middleware runs **before** routing. It resolves the plane from
//! `Host` + path (`planes::resolve_plane`) and:
//!   * `Mock` (P1) — short-circuits to the interceptor **without ever entering
//!     the inner router**. This is the load-bearing isolation guarantee (R6,
//!     AC-6): a mock-host request — incl. `/api/...` — can NEVER reach the P2
//!     management routes or the P3 SPA fallback. (Interceptor body: sks.15.)
//!   * `Api` (P2) / `Ui` (P3) — falls through to the inner router, which holds
//!     the real `/api/**` management routes (sks.14), `/healthz`, and (sks.21 /
//!     sks.24) the feed + SPA. The `X-HookBox-Plane` observability header is
//!     injected on the response either way.

use std::collections::BTreeMap;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderValue, Request, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use http_body_util::BodyExt;
use tower_http::catch_panic::CatchPanicLayer;

use crate::interceptor::engine;
use crate::planes::{resolve_plane, Plane, PlaneResult};
use crate::routes::{api_router, feed, healthz, share_router, tunnel_ws};
use crate::state::AppState;

fn plane_str(p: &Plane) -> &'static str {
    match p {
        Plane::Mock => "mock",
        Plane::Api => "api",
        Plane::Ui => "ui",
    }
}

fn request_host(req: &Request<Body>) -> String {
    req.headers()
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.uri().authority().map(|a| a.to_string()))
        .unwrap_or_default()
}

/// Parse the raw query string into a key→value map (last value wins, matching
/// the Python `dict(request.query_params)`).
fn parse_query(uri: &axum::http::Uri) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    if let Some(q) = uri.query() {
        for pair in q.split('&') {
            if pair.is_empty() {
                continue;
            }
            let mut it = pair.splitn(2, '=');
            let k = it.next().unwrap_or("");
            let v = it.next().unwrap_or("");
            let kd = urlencoding_decode(k);
            let vd = urlencoding_decode(v);
            map.insert(kd, vd);
        }
    }
    map
}

/// Minimal percent-decode (`+` → space, `%XX` → byte). Lossy on bad escapes.
fn urlencoding_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b);
                    i += 2;
                } else {
                    out.push(b'%');
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Run the P1 interceptor for a mock request: buffer the (capped) body, then
/// hand off to `engine::handle_mock`.
async fn run_interceptor(state: &AppState, result: &PlaneResult, req: Request<Body>) -> Response {
    let token = result.token.clone().unwrap_or_default();
    let mock_path = result.mock_path.clone().unwrap_or_else(|| "/".to_string());
    let method = req.method().as_str().to_string();
    let headers = req.headers().clone();
    let query = parse_query(req.uri());

    // Early ingest cap via Content-Length, then buffer the body.
    let cap = state.cfg.max_ingest_body_bytes;
    if let Some(cl) = headers
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok())
    {
        if cl > cap {
            return oversize_response(&token);
        }
    }
    // Bound the read at `cap` so a chunked request with no Content-Length can't
    // buffer past the limit before the size check (AC-S18 — reject before fully
    // buffering). `Limited` errors with `LengthLimitError` once `cap` is exceeded.
    let body = match http_body_util::Limited::new(req.into_body(), cap)
        .collect()
        .await
    {
        Ok(c) => c.to_bytes().to_vec(),
        Err(e)
            if e.downcast_ref::<http_body_util::LengthLimitError>()
                .is_some() =>
        {
            return oversize_response(&token);
        }
        // A genuine transport read error → treat as an empty body (prior behavior).
        Err(_) => Vec::new(),
    };
    if body.len() > cap {
        return oversize_response(&token);
    }

    engine::handle_mock(state, &token, &mock_path, &method, headers, query, body).await
}

fn oversize_response(token: &str) -> Response {
    let mut r = (
        StatusCode::PAYLOAD_TOO_LARGE,
        axum::Json(serde_json::json!({"error":"payload_too_large","detail":"Request body exceeds the configured limit."})),
    )
        .into_response();
    if let Ok(hv) = HeaderValue::from_str(token) {
        r.headers_mut().insert("x-hookbox-endpoint", hv);
    }
    r
}

/// Plane-resolution middleware. Short-circuits P1 to the interceptor; otherwise
/// runs the inner router. Adds `X-HookBox-Plane` on the way out.
pub async fn plane_dispatch(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let host = request_host(&req);
    let path = req.uri().path().to_string();
    let result = resolve_plane(&host, &path, &state.cfg.mock_domain, &state.cfg.app_hosts);
    let plane_value = plane_str(&result.plane);

    let mut resp = match result.plane {
        // Mock host (or /e fallback) never enters the management/UI router.
        Plane::Mock => run_interceptor(&state, &result, req).await,
        // Management API (P2) + UI/feed/health (P3) route normally.
        _ => next.run(req).await,
    };

    if let Ok(hv) = HeaderValue::from_str(plane_value) {
        resp.headers_mut().insert("x-hookbox-plane", hv);
    }
    resp
}

/// Build the top-level app: the plane-dispatch middleware wrapping the inner
/// router (the real `/api` management routes + `/healthz` + UI fallback). The
/// middleware short-circuits P1 so the mock catch-all can never shadow these.
///
/// `CatchPanicLayer` is added **outermost** (AC-S18): F7's truncation path now
/// routes attacker/upstream-controlled bytes, and this turns any future panic
/// into a 500 instead of a dropped connection, without changing the bytes of
/// any non-panicking response (AC-72). No `TraceLayer` is added here — AC-S6
/// requires the app's own logging to never see a share code or the full path.
pub fn build_app(state: AppState) -> Router {
    let inner = Router::new()
        .merge(api_router())
        // F4 share links: three owner routes + two PUBLIC unauthenticated
        // routes (§5.1/§5.2). Merged here, not layered separately, so no
        // TraceLayer or other logging middleware can end up wrapping only
        // this router and start logging a share code (AC-S6).
        .merge(share_router())
        .route("/healthz", get(healthz))
        // P3 live feed (owner-gated via ?cap=).
        .route("/ws/:token", get(feed::ws_handler))
        .route("/sse/:token", get(feed::sse_handler))
        // P3 tunnel control channel (bind auth before accept).
        .route("/ws/tunnel/:slug", get(tunnel_ws::tunnel_ws_handler))
        // P3 SPA: serve dist/ with index.html fallback (R6 — never shadows P1/P2).
        .fallback(crate::routes::spa::serve_spa);
    let inner = add_test_only_routes(inner);

    inner
        .layer(from_fn_with_state(state.clone(), plane_dispatch))
        .with_state(state)
        .layer(CatchPanicLayer::new())
}

/// No-op in every real build.
#[cfg(not(test))]
fn add_test_only_routes(router: Router<AppState>) -> Router<AppState> {
    router
}

/// A route that unconditionally panics, mounted through the exact same router
/// (and therefore the exact same layer stack — `plane_dispatch` then
/// `CatchPanicLayer`) as production — so the AC-S18 regression test proves
/// the real stack survives a panic, not a hand-built stand-in. Compiled out
/// of every non-test build.
#[cfg(test)]
fn add_test_only_routes(router: Router<AppState>) -> Router<AppState> {
    router.route("/__test_panic", get(test_panic_handler))
}

#[cfg(test)]
async fn test_panic_handler() -> Response {
    panic!("AC-S18 test-only panic")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::db;
    use crate::ids::{gen_owner_secret, hash_email, hash_secret};
    use axum::http::Request;
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use sqlx::SqlitePool;
    use tower::ServiceExt;

    async fn app_with_owner() -> (Router, SqlitePool, String, String) {
        // Build the Config under the global env lock so the set_var -> from_env
        // window is atomic w.r.t. other env-mutating tests (AC-55 determinism).
        let cfg = {
            let _guard = crate::testutil::env_lock();
            std::env::set_var("MOCK_DOMAIN", "mock.local");
            std::env::set_var("APP_HOST", "app.local");
            Config::from_env()
        };
        let pool = db::pool(":memory:").await.unwrap();
        db::migrate(&pool).await.unwrap();
        let owner_id = hash_email("a@b.com");
        let secret = gen_owner_secret(32);
        sqlx::query("INSERT INTO owners (owner_id, email, secret_hash) VALUES (?, ?, ?)")
            .bind(&owner_id)
            .bind("a@b.com")
            .bind(hash_secret(&secret))
            .execute(&pool)
            .await
            .unwrap();
        let state = AppState::new(pool.clone(), cfg);
        (build_app(state), pool, owner_id, secret)
    }

    async fn req(
        app: &Router,
        method: &str,
        host: &str,
        path: &str,
        bearer: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value, Option<String>) {
        let mut b = Request::builder()
            .method(method)
            .uri(path)
            .header("host", host);
        if let Some(t) = bearer {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        let request = if let Some(j) = body {
            b.header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&j).unwrap()))
                .unwrap()
        } else {
            b.body(Body::empty()).unwrap()
        };
        let resp = app.clone().oneshot(request).await.unwrap();
        let status = resp.status();
        let plane = resp
            .headers()
            .get("x-hookbox-plane")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        (status, v, plane)
    }

    #[tokio::test]
    async fn unauthed_api_is_401_session_is_open() {
        let (app, _pool, _oid, _secret) = app_with_owner().await;
        let (s, body, plane) = req(&app, "GET", "app.local", "/api/endpoints", None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        assert_eq!(plane.as_deref(), Some("api"));
        assert_eq!(body["error"], json!("unauthorized"));
        // session is open (no auth) — bad email still parses to 422 via the handler.
        let (s2, _b2, _) = req(
            &app,
            "POST",
            "app.local",
            "/api/session",
            None,
            Some(json!({"email":"nope"})),
        )
        .await;
        assert_eq!(s2, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn endpoint_crud_round_trip() {
        let (app, _pool, _oid, secret) = app_with_owner().await;
        // create
        let (s, body, _) = req(
            &app,
            "POST",
            "app.local",
            "/api/endpoints",
            Some(&secret),
            Some(json!({"name":"demo"})),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED);
        let token = body["token"].as_str().unwrap().to_string();
        assert_eq!(body["chaos_mode"], json!("error")); // OQ-2 default present
        assert_eq!(body["default_mode"], json!("mock_404"));
        assert!(body.as_object().unwrap().contains_key("last_hit")); // null key present
                                                                     // list
        let (s, body, plane) = req(
            &app,
            "GET",
            "app.local",
            "/api/endpoints",
            Some(&secret),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(plane.as_deref(), Some("api"));
        assert_eq!(body.as_array().unwrap().len(), 1);
        // patch: clamp latency, validate target_url, set chaos_mode
        let (s, body, _) = req(&app, "PATCH", "app.local", &format!("/api/endpoints/{token}"), Some(&secret),
            Some(json!({"latency_ms": 999999, "chaos_mode":"dropout", "target_url":"https://up.example.com"}))).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(body["latency_ms"], json!(10000)); // clamped
        assert_eq!(body["chaos_mode"], json!("dropout"));
        assert_eq!(body["target_url"], json!("https://up.example.com"));
        // patch bad target_url -> 422
        let (s, _b, _) = req(
            &app,
            "PATCH",
            "app.local",
            &format!("/api/endpoints/{token}"),
            Some(&secret),
            Some(json!({"target_url":"ftp://bad"})),
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
        // delete -> tombstone, 200 Message
        let (s, body, _) = req(
            &app,
            "DELETE",
            "app.local",
            &format!("/api/endpoints/{token}"),
            Some(&secret),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(body["success"], json!(true));
    }

    #[tokio::test]
    async fn rules_crud_and_204_and_order() {
        let (app, _pool, _oid, secret) = app_with_owner().await;
        let (_, ep, _) = req(
            &app,
            "POST",
            "app.local",
            "/api/endpoints",
            Some(&secret),
            Some(json!({})),
        )
        .await;
        let token = ep["token"].as_str().unwrap().to_string();
        // create two rules with different priority
        let (s, r1, _) = req(
            &app,
            "POST",
            "app.local",
            &format!("/api/endpoints/{token}/rules"),
            Some(&secret),
            Some(json!({"priority": 50, "match": {"method":"GET"}})),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED);
        assert_eq!(r1["match"]["method"], json!("GET"));
        assert!(r1.as_object().unwrap().contains_key("webhook_action"));
        let (_, _r2, _) = req(
            &app,
            "POST",
            "app.local",
            &format!("/api/endpoints/{token}/rules"),
            Some(&secret),
            Some(json!({"priority": 10})),
        )
        .await;
        // list ordered by priority,id
        let (s, list, _) = req(
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
        assert_eq!(arr[0]["priority"], json!(10));
        assert_eq!(arr[1]["priority"], json!(50));
        let rid = r1["id"].as_i64().unwrap();
        // delete -> 204 no body
        let (s, body, _) = req(
            &app,
            "DELETE",
            "app.local",
            &format!("/api/endpoints/{token}/rules/{rid}"),
            Some(&secret),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NO_CONTENT);
        assert!(body.is_null());
        // get deleted -> 404
        let (s, _b, _) = req(
            &app,
            "GET",
            "app.local",
            &format!("/api/endpoints/{token}/rules/{rid}"),
            Some(&secret),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn requests_limit_validation_and_state_collections() {
        let (app, _pool, _oid, secret) = app_with_owner().await;
        let (_, ep, _) = req(
            &app,
            "POST",
            "app.local",
            "/api/endpoints",
            Some(&secret),
            Some(json!({})),
        )
        .await;
        let token = ep["token"].as_str().unwrap().to_string();
        // limit out of range -> 422
        let (s, _b, _) = req(
            &app,
            "GET",
            "app.local",
            &format!("/api/endpoints/{token}/requests?limit=500"),
            Some(&secret),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
        // valid -> 200 empty list
        let (s, body, _) = req(
            &app,
            "GET",
            "app.local",
            &format!("/api/endpoints/{token}/requests?limit=10"),
            Some(&secret),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(body.as_array().unwrap().len(), 0);
        // state peek -> {"state": {}}
        let (s, body, _) = req(
            &app,
            "GET",
            "app.local",
            &format!("/api/endpoints/{token}/state"),
            Some(&secret),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(body["state"], json!({}));
        // collection unsafe name -> 422 invalid_collection
        let (s, body, _) = req(
            &app,
            "GET",
            "app.local",
            &format!("/api/endpoints/{token}/collections/bad%20name"),
            Some(&secret),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"], json!("invalid_collection"));
        // valid collection -> {"items": []}
        let (s, body, _) = req(
            &app,
            "GET",
            "app.local",
            &format!("/api/endpoints/{token}/collections/widgets"),
            Some(&secret),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(body["items"], json!([]));
    }

    #[tokio::test]
    async fn mock_host_api_never_reaches_management() {
        let (app, _pool, _oid, secret) = app_with_owner().await;
        // A mock-host /api request must be P1 (mock), not the management API,
        // even with a valid Bearer. The token `tok123` is unknown -> 404
        // unknown_endpoint (the interceptor), NOT a JSON endpoint list.
        let (s, body, plane) = req(
            &app,
            "GET",
            "tok123.mock.local",
            "/api/endpoints",
            Some(&secret),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        assert_eq!(plane.as_deref(), Some("mock"));
        assert_eq!(body["error"], json!("unknown_endpoint"));
    }

    #[tokio::test]
    async fn mock_host_served_by_rule_and_default_404() {
        let (app, pool, _oid, _secret) = app_with_owner().await;
        // Create an endpoint owned by our owner with a GET /* rule.
        let oid = crate::ids::hash_email("a@b.com");
        sqlx::query("INSERT INTO endpoints (token, owner_id) VALUES ('tokLIVE001', ?)")
            .bind(&oid)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO mock_rules (token, priority, enabled, match_json, response_json, state_writes_json)
             VALUES ('tokLIVE001', 100, 1, '{\"method\":\"GET\",\"path\":\"/hello\"}',
                     '{\"status_code\":201,\"content_type\":\"application/json\",\"body_template\":\"{\\\"ok\\\":true}\"}', '[]')",
        ).execute(&pool).await.unwrap();

        // Matching rule -> 201 with identifying headers.
        let request = Request::builder()
            .method("GET")
            .uri("/hello")
            .header("host", "tokLIVE001.mock.local")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(request).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        assert_eq!(resp.headers().get("x-hookbox-served-by").unwrap(), "rule");
        assert_eq!(
            resp.headers().get("x-hookbox-endpoint").unwrap(),
            "tokLIVE001"
        );
        assert_eq!(resp.headers().get("x-hookbox-rule-id").unwrap(), "1");

        // Non-matching path -> default mock_404.
        let request = Request::builder()
            .method("GET")
            .uri("/nope")
            .header("host", "tokLIVE001.mock.local")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(request).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            resp.headers().get("x-hookbox-served-by").unwrap(),
            "default"
        );
    }

    #[tokio::test]
    async fn non_owner_endpoint_is_404() {
        let (app, pool, _oid, secret) = app_with_owner().await;
        // create owner B's endpoint directly.
        let b_id = hash_email("b@c.com");
        sqlx::query("INSERT INTO owners (owner_id, email, secret_hash) VALUES (?,?,?)")
            .bind(&b_id)
            .bind("b@c.com")
            .bind(hash_secret("xyz"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO endpoints (token, owner_id) VALUES (?,?)")
            .bind("tokBBBBBBB")
            .bind(&b_id)
            .execute(&pool)
            .await
            .unwrap();
        // owner A (secret) cannot see it -> 404 not 403.
        let (s, body, _) = req(
            &app,
            "GET",
            "app.local",
            "/api/endpoints/tokBBBBBBB",
            Some(&secret),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], json!("not_found"));
    }

    #[tokio::test]
    async fn session_creates_owner_and_primary_endpoint() {
        let (app, _pool, _oid, _secret) = app_with_owner().await;
        let (s, body, plane) = req(
            &app,
            "POST",
            "app.local",
            "/api/session",
            None,
            Some(json!({"email":"new@user.com"})),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(plane.as_deref(), Some("api"));
        assert!(body["owner_secret"].as_str().unwrap().len() >= 40);
        assert_eq!(body["endpoints"].as_array().unwrap().len(), 1);
        assert!(body["primary"]["token"].is_string());
    }

    // AC-S18: a handler that panics, mounted through the real layer stack
    // (`plane_dispatch` then the outermost `CatchPanicLayer`), returns 500 —
    // not a dropped connection — and the server keeps serving requests
    // afterward. Also confirms a non-panicking response's bytes are
    // untouched by the new layer (AC-72): the same assertions
    // `mock_host_served_by_rule_and_default_404` makes on `x-hookbox-*`
    // headers and status codes still hold with `CatchPanicLayer` in place.
    #[tokio::test]
    async fn panic_becomes_500_and_next_request_still_served() {
        let (app, _pool, _oid, _secret) = app_with_owner().await;

        let panic_request = Request::builder()
            .method("GET")
            .uri("/__test_panic")
            .header("host", "app.local")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(panic_request).await.unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // The connection/server survives: the very next request is served
        // normally, with the plane header still attached.
        let (s, body, plane) = req(&app, "GET", "app.local", "/api/endpoints", None, None).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        assert_eq!(plane.as_deref(), Some("api"));
        assert_eq!(body["error"], json!("unauthorized"));
    }
}
