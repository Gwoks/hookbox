//! SPA static serving + index.html fallback — mirrors the reference `spa.rs`
//! (§Component design; AC-53, R6).
//!
//! Serves built assets from `STATIC_DIR` (default `dist/`). Any app-host
//! non-`/api` non-mock path that does not map to a real file falls back to
//! `index.html` so the client router can render it. The plane middleware runs
//! first, so a mock-host request never reaches here (R6) and `/api` on a mock
//! host stays P1.

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderName, HeaderValue, Request, StatusCode};
use axum::response::Response;

use crate::state::AppState;

/// Security headers scoped to the PUBLIC share viewer path (`/s/<code>`)
/// ONLY (hookbox-mun.40, AC-S5, AC-S26). Mirrors `deploy/nginx.conf`'s
/// `location /s/` block exactly, so the two topologies — behind nginx, or
/// the bare backend, per the shipped nginx-less `docker-compose.yml` and
/// `cargo run` — agree, and nginx becomes belt-and-braces rather than the
/// only place these are ever applied. The dashboard SPA (served at every
/// other app-host path) does NOT get `X-Robots-Tag: noindex, nofollow` —
/// nginx's `location /` carries none of these directives either, and the
/// dashboard is an authenticated operator tool, not the anonymous,
/// crawlable-by-default resource `X-Robots-Tag`/framing protection exists to
/// police for the share viewer.
const SHARE_VIEWER_HEADERS: [(&str, &str); 4] = [
    ("referrer-policy", "no-referrer"),
    ("x-robots-tag", "noindex, nofollow"),
    ("x-content-type-options", "nosniff"),
    ("x-frame-options", "DENY"),
];

fn apply_share_viewer_headers(resp: &mut Response) {
    for (name, value) in SHARE_VIEWER_HEADERS {
        resp.headers_mut().insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
}

fn content_type_for(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "map" => "application/json",
        _ => "application/octet-stream",
    }
}

/// Serve a static asset under `static_dir`, falling back to `index.html`.
/// Rejects path traversal (`..`).
pub async fn serve_spa(State(state): State<AppState>, req: Request<Body>) -> Response {
    let dir = state.cfg.static_dir.clone();
    let raw_path = req.uri().path();
    // hookbox-mun.40: the share viewer's document lives under this prefix
    // regardless of whether it's served from a real file or the SPA
    // fallback below.
    let is_share_viewer = raw_path.starts_with("/s/");
    // Reject traversal.
    if raw_path.contains("..") {
        return not_found();
    }
    let rel = raw_path.trim_start_matches('/');
    let candidate = if rel.is_empty() {
        format!("{dir}/index.html")
    } else {
        format!("{dir}/{rel}")
    };

    // Serve a real file if it exists.
    if let Ok(bytes) = tokio::fs::read(&candidate).await {
        let mut resp = file_response(&candidate, bytes);
        if is_share_viewer {
            apply_share_viewer_headers(&mut resp);
        }
        return resp;
    }
    // SPA fallback: index.html for client routes.
    let index = format!("{dir}/index.html");
    match tokio::fs::read(&index).await {
        Ok(bytes) => {
            let mut resp = file_response(&index, bytes);
            if is_share_viewer {
                apply_share_viewer_headers(&mut resp);
            }
            resp
        }
        Err(_) => not_found(),
    }
}

fn file_response(path: &str, bytes: Vec<u8>) -> Response {
    let ct = content_type_for(path);
    let mut resp = Response::new(Body::from(bytes));
    resp.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
    resp
}

fn not_found() -> Response {
    let mut resp = Response::new(Body::from("Not Found"));
    *resp.status_mut() = StatusCode::NOT_FOUND;
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::db;

    async fn state_with_static_dir(dir: &str) -> AppState {
        let pool = db::pool(":memory:").await.unwrap();
        db::migrate(&pool).await.unwrap();
        let mut cfg = Config::from_env();
        cfg.static_dir = dir.to_string();
        AppState::new(pool, cfg)
    }

    // hookbox-mun.40: a live GET /s/<CODE> served by the backend's OWN SPA
    // handler (no nginx in front, as in the shipped nginx-less
    // docker-compose.yml / `cargo run`) must carry all four AC-S5/AC-S26
    // headers. The dashboard SPA (any other app-host path) must NOT — that
    // matches `deploy/nginx.conf`'s `location /` (which carries none of
    // these) so the two topologies agree.
    #[tokio::test]
    async fn share_viewer_path_carries_security_headers_dashboard_path_does_not() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<html>ok</html>").unwrap();
        let state = state_with_static_dir(dir.path().to_str().unwrap()).await;

        let req = Request::builder()
            .uri("/s/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
            .body(Body::empty())
            .unwrap();
        let resp = serve_spa(State(state.clone()), req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("referrer-policy").unwrap(),
            "no-referrer"
        );
        assert_eq!(
            resp.headers().get("x-robots-tag").unwrap(),
            "noindex, nofollow"
        );
        assert_eq!(
            resp.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");

        let dashboard_req = Request::builder()
            .uri("/dashboard")
            .body(Body::empty())
            .unwrap();
        let dashboard_resp = serve_spa(State(state), dashboard_req).await;
        assert_eq!(dashboard_resp.status(), StatusCode::OK);
        assert!(dashboard_resp.headers().get("x-robots-tag").is_none());
        assert!(dashboard_resp.headers().get("x-frame-options").is_none());
        assert!(dashboard_resp
            .headers()
            .get("x-content-type-options")
            .is_none());
        assert!(dashboard_resp.headers().get("referrer-policy").is_none());
    }
}
