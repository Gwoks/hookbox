//! Response plumbing shared by every engine stage: header insertion, JSON
//! bodies, the identifying + CORS headers, and the F7 body capture.

use std::collections::BTreeMap;

use axum::body::{to_bytes, Body, Bytes, HttpBody};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::Value;

use super::exchange::Exchange;
use crate::interceptor::cors;
use crate::interceptor::mock_error::MockError;
use crate::models::ServedBy;
use crate::net::headers::{header_pairs, try_header};
use crate::rule_cache::CompiledEndpoint;

/// Inbound headers as a lower-cased-name map (last value wins).
pub(super) fn lower_headers(headers: &HeaderMap) -> BTreeMap<String, String> {
    header_pairs(headers)
        .map(|(k, v)| (k.to_ascii_lowercase(), v.to_string()))
        .collect()
}

/// Insert one header, silently skipping an invalid name or value.
pub(super) fn insert_header(headers: &mut HeaderMap, name: &str, value: &str) {
    if let Some((n, v)) = try_header(name, value) {
        headers.insert(n, v);
    }
}

/// A response with `status` (200 if out of range) and `body`.
pub(super) fn with_status(status: u16, body: Body) -> Response {
    let mut resp = Response::new(body);
    *resp.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
    resp
}

/// A JSON body (`serde_json` bytes) with `content-type: application/json`.
pub(super) fn json_response(status: u16, body: &Value) -> Response {
    let bytes = serde_json::to_vec(body).unwrap_or_default();
    let mut resp = with_status(status, Body::from(bytes));
    insert_header(resp.headers_mut(), "content-type", "application/json");
    resp
}

/// The 204 CORS preflight (auto-CORS headers when enabled).
pub(super) fn preflight(ex: &Exchange<'_>) -> Response {
    let mut resp = with_status(StatusCode::NO_CONTENT.as_u16(), Body::empty());
    for (k, v) in cors::preflight_headers(&ex.req.headers, ex.ep.cors_enabled) {
        insert_header(resp.headers_mut(), &k, &v);
    }
    resp
}

/// Attach `X-HookBox-*` identifying headers + auto-CORS (P1).
pub(super) fn identified(
    mut resp: Response,
    token: &str,
    served_by: ServedBy,
    rule_id: Option<i64>,
    ep: &CompiledEndpoint,
    headers_lower: &BTreeMap<String, String>,
) -> Response {
    let headers = resp.headers_mut();
    insert_header(headers, "x-hookbox-endpoint", token);
    insert_header(headers, "x-hookbox-served-by", served_by.as_str());
    if let Some(id) = rule_id {
        insert_header(headers, "x-hookbox-rule-id", &id.to_string());
    }
    for (k, v) in cors::response_headers(headers_lower, ep.cors_enabled) {
        insert_header(headers, &k, &v);
    }
    resp
}

/// 404 `unknown_endpoint` / 410 `endpoint_gone`, carrying only
/// `X-HookBox-Endpoint` (never traced).
pub(super) fn unknown_or_gone(token: &str, gone: bool) -> Response {
    let error = if gone {
        MockError::EndpointGone
    } else {
        MockError::UnknownEndpoint
    };
    let mut resp = error.into_response();
    insert_header(resp.headers_mut(), "x-hookbox-endpoint", token);
    resp
}

/// F7 (architecture.md §2.10.1): buffer a finished mock response's body so the
/// trace can persist the exact bytes the client will receive, returning an
/// equivalent response plus those bytes.
///
/// Allocation-free for the payload: every mock-plane body is built from an
/// owned buffer (a rendered template, `serde_json::to_vec`, the MITM reply,
/// the tunnel's decoded reply, or `Body::empty()`), so it is a SINGLE-FRAME
/// body — `to_bytes` hands back the same `Bytes` buffer and `Bytes::clone` is
/// a refcount bump, not a copy. Nothing here can block: the frame is already
/// resolved, so the `.await` never yields.
///
/// The `size_hint().exact()` guard is a forward-compatibility fuse: if a
/// streaming mock body is ever introduced, this returns the response
/// untouched and captures nothing rather than buffering an unbounded stream.
pub(super) async fn capture_response_body(resp: Response) -> (Response, Bytes) {
    let (parts, body) = resp.into_parts();
    let Some(len) = body.size_hint().exact() else {
        tracing::warn!("mock response body has no exact size; response_body not captured");
        return (Response::from_parts(parts, body), Bytes::new());
    };
    if len == 0 {
        // 204 CORS preflight, empty 204 CRUD, chaos dropout — nothing to capture.
        return (Response::from_parts(parts, body), Bytes::new());
    }
    match to_bytes(body, len as usize).await {
        Ok(bytes) => (
            Response::from_parts(parts, Body::from(bytes.clone())),
            bytes,
        ),
        Err(e) => {
            // Unreachable for an in-memory body (`len` is exact, so the limit
            // cannot trip, and these bodies are infallible). If it ever fires
            // the body is already lost, so the only honest option is an
            // empty one.
            tracing::error!("failed to buffer mock response body: {e}");
            (Response::from_parts(parts, Body::empty()), Bytes::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    // F7 AC-73(d3): the capture helper on a 5 MB body (the MITM_MAX_BODY_BYTES
    // worst case) must be zero-copy, and the rebuilt response's bytes must be
    // byte-equal to the input.
    //
    // Zero-copy is proven by timing, not by a fixed millisecond budget — a
    // fixed `< 1ms` bound (hookbox-mun.32) has under 1x margin against warm
    // ~0.3ms runs and flakes ~6% of the time on a cold/loaded/shared runner,
    // because it is racing wall-clock scheduling noise, not the code path.
    //
    // Instead we compare `capture_response_body` against a same-process,
    // same-buffer, same-run explicit `Vec::clone` (a real memcpy) of the
    // identical 5 MB payload. That comparison is scale-free: a slow or
    // loaded runner slows both timings down together, so the RATIO is stable
    // even when the absolute numbers are not. A full-copy implementation of
    // `capture_response_body` would cost about the same as the memcpy
    // control, so requiring capture to be well under it (< 25%) still fails
    // a regression that reintroduces a copy, while a min-of-N sample (a
    // warm-up run discarded, then the fastest of several timed runs kept)
    // removes first-invocation/cold-cache noise from BOTH sides.
    #[tokio::test]
    async fn capture_response_body_5mb_is_faster_than_a_full_copy_and_byte_equal() {
        const ITERATIONS: usize = 20;
        let payload = vec![b'x'; 5_000_000];

        // Warm-up: pay for allocator/CPU-frequency-scaling/page-fault costs
        // once, outside any measured sample, on both code paths.
        {
            let resp = with_status(200, Body::from(payload.clone()));
            let _ = capture_response_body(resp).await;
            let warm_control: Vec<u8> = payload.clone();
            std::hint::black_box(&warm_control);
        }

        let mut capture_min = Duration::MAX;
        let mut captured_bytes = Bytes::new();
        let mut rebuilt_bytes = Bytes::new();
        for _ in 0..ITERATIONS {
            let resp = with_status(200, Body::from(payload.clone()));
            let started = Instant::now();
            let (rebuilt, bytes) = capture_response_body(resp).await;
            let elapsed = started.elapsed();
            if elapsed < capture_min {
                capture_min = elapsed;
                captured_bytes = bytes;
                rebuilt_bytes = to_bytes(rebuilt.into_body(), usize::MAX).await.unwrap();
            }
        }

        let mut copy_min = Duration::MAX;
        for _ in 0..ITERATIONS {
            let started = Instant::now();
            let control: Vec<u8> = payload.clone();
            let elapsed = started.elapsed();
            std::hint::black_box(&control);
            copy_min = copy_min.min(elapsed);
        }

        assert_eq!(captured_bytes.len(), payload.len());
        assert_eq!(captured_bytes.as_ref(), payload.as_slice());
        // The rebuilt response's own body is byte-equal too.
        assert_eq!(rebuilt_bytes.as_ref(), payload.as_slice());

        // A full-copy implementation would cost roughly `copy_min`; the
        // zero-copy path must be clearly (>= 4x) faster than that, on the
        // same machine, same run, same buffer — so this cannot pass a
        // full-copy regression and cannot flake from an absolute-time budget.
        assert!(
            capture_min.saturating_mul(4) < copy_min,
            "capture_response_body best-of-{ITERATIONS} ({capture_min:?}) is not clearly \
             faster than a best-of-{ITERATIONS} explicit 5MB Vec::clone ({copy_min:?}); \
             expected < 25% of the copy control, which suggests a full copy crept back in"
        );
    }

    #[tokio::test]
    async fn capture_response_body_empty_is_none_and_untouched() {
        let resp = with_status(204, Body::empty());
        let (rebuilt, bytes) = capture_response_body(resp).await;
        assert!(bytes.is_empty());
        assert_eq!(rebuilt.status(), StatusCode::NO_CONTENT);
    }
}
