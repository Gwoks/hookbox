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
    http::{Request, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::{IntoResponse, Response},
    Router,
};
use http_body_util::BodyExt;
use tower_http::catch_panic::CatchPanicLayer;

use crate::interceptor::engine;
use crate::net::headers::try_header;
use crate::planes::{resolve_plane, Plane, PlaneResult};
use crate::routes;
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
    if let Some((name, value)) = try_header("x-hookbox-endpoint", token) {
        r.headers_mut().insert(name, value);
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

    if let Some((name, value)) = try_header("x-hookbox-plane", plane_value) {
        resp.headers_mut().insert(name, value);
    }
    resp
}

/// Build the top-level app: the plane-dispatch middleware wrapping the inner
/// router (`routes::router()` — the single route table). The middleware
/// short-circuits P1 so the mock catch-all can never shadow these.
///
/// `CatchPanicLayer` is added **outermost** (AC-S18): F7's truncation path now
/// routes attacker/upstream-controlled bytes, and this turns any future panic
/// into a 500 instead of a dropped connection, without changing the bytes of
/// any non-panicking response (AC-72). No `TraceLayer` is added here — AC-S6
/// requires the app's own logging to never see a share code or the full path.
pub fn build_app(state: AppState) -> Router {
    add_test_only_routes(routes::router())
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
    router.route("/__test_panic", axum::routing::get(test_panic_handler))
}

#[cfg(test)]
async fn test_panic_handler() -> Response {
    panic!("AC-S18 test-only panic")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{memory_pool, test_config};
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    // Route-level scenarios (session, endpoint/rule CRUD, paging, owner
    // scoping, mock-host isolation) live in the integration suite
    // (`tests/api`, `tests/wire_contract`), which drives this same
    // `build_app` stack; only router-internal checks stay here.

    fn app_get(path: &str) -> Request<Body> {
        Request::builder()
            .method("GET")
            .uri(path)
            .header("host", "app.local")
            .body(Body::empty())
            .unwrap()
    }

    // AC-S18: a handler that panics, mounted through the real layer stack
    // (`plane_dispatch` then the outermost `CatchPanicLayer`), returns 500 —
    // not a dropped connection — and the server keeps serving requests
    // afterward. The integration suite's exact status/`x-hookbox-*`/byte
    // assertions run through this same stack, so `CatchPanicLayer` leaves
    // every non-panicking response untouched (AC-72).
    #[tokio::test]
    async fn panic_becomes_500_and_next_request_still_served() {
        let app = build_app(AppState::new(memory_pool().await, test_config()));

        let resp = app.clone().oneshot(app_get("/__test_panic")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

        // The connection/server survives: the very next request is served
        // normally, with the plane header still attached.
        let resp = app
            .clone()
            .oneshot(app_get("/api/endpoints"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(resp.headers().get("x-hookbox-plane").unwrap(), "api");
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"], json!("unauthorized"));
    }

    /// Oracle for the mock-plane query parser (S0 safety net, hookbox-a73):
    /// a later phase swaps the hand-rolled decoder for `form_urlencoded` and
    /// must reproduce every row here unchanged.
    #[test]
    fn parse_query_oracle() {
        let cases: [(&str, &[(&str, &str)]); 13] = [
            ("a=1&b=2", &[("a", "1"), ("b", "2")]),
            ("a=x+y", &[("a", "x y")]),
            ("a=%41", &[("a", "A")]),
            ("a=%4", &[("a", "%4")]),
            ("a=%zz", &[("a", "%zz")]),
            ("a=%%41", &[("a", "%A")]),
            ("=v", &[("", "v")]),
            ("a", &[("a", "")]),
            ("&&a=1&&", &[("a", "1")]),
            ("a=1&a=2", &[("a", "2")]),
            ("a=%E2%82%AC", &[("a", "\u{20ac}")]),
            ("a=%FF", &[("a", "\u{fffd}")]),
            ("a=b=c", &[("a", "b=c")]),
        ];
        for (query, expected) in cases {
            let uri: axum::http::Uri = format!("/p?{query}").parse().unwrap();
            let want: BTreeMap<String, String> = expected
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            assert_eq!(parse_query(&uri), want, "query `{query}`");
        }
    }
}
