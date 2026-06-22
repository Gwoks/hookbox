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
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::response::Response;

use crate::state::AppState;

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
        return file_response(&candidate, bytes);
    }
    // SPA fallback: index.html for client routes.
    let index = format!("{dir}/index.html");
    match tokio::fs::read(&index).await {
        Ok(bytes) => file_response(&index, bytes),
        Err(_) => not_found(),
    }
}

fn file_response(path: &str, bytes: Vec<u8>) -> Response {
    let ct = content_type_for(path);
    let mut resp = Response::new(Body::from(bytes));
    resp.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static(ct));
    resp
}

fn not_found() -> Response {
    let mut resp = Response::new(Body::from("Not Found"));
    *resp.status_mut() = StatusCode::NOT_FOUND;
    resp
}
