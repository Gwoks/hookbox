//! Core interceptor engine — PORT of `app/interceptor/engine.py` (§5.5).
//!
//! `handle_mock` owns the frozen resolution order
//! (OPTIONS → rule → Auto-CRUD → tunnel → MITM → default) and the conditions
//! wrap (rate-limit (429) → chaos (5xx/drop) → latency (sleep)). The endpoint
//! config + compiled rules come from the in-process `rule_cache` (no per-request
//! DB read on a match); per-endpoint state is read only when some rule gates on
//! it (lazy). The trace write + feed publish run OFF the response path (spawned,
//! never awaited) so a slow/failed trace never delays/fails the mock response.
//!
//! Identifying headers on every P1 response: `X-HookBox-Endpoint`,
//! `X-HookBox-Served-By`, `X-HookBox-Rule-Id` (only when a rule matched),
//! `X-HookBox-Truncated` (MITM only) + auto-CORS (P1, when enabled).
//!
//! Stages, one module each: `exchange` (the per-request context and its single
//! `finish` exit), `rule` (a matched rule), `unmatched` (CRUD / tunnel / MITM /
//! default), `wrap` (the conditions), `respond` (response plumbing), `trace`
//! (the off-path persist) and `echo` (the echo payload + its persisted
//! redaction).

use std::collections::BTreeMap;

use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};

use crate::interceptor::mock_error::MockError;
use crate::models::ServedBy;
use crate::rule_cache::Resolved;
use crate::state::AppState;

mod echo;
mod exchange;
mod respond;
mod rule;
mod trace;
mod unmatched;
mod wrap;

use exchange::{Exchange, MockRequest};
use respond::{identified, lower_headers, unknown_or_gone};
use wrap::Wrapped;

/// Build the §5.5 P1 response for `(token, mock_path)`. `headers`/`query`/`body`
/// are the inbound request parts.
pub async fn handle_mock(
    state: &AppState,
    token: &str,
    mock_path: &str,
    method: &str,
    headers: HeaderMap,
    query: BTreeMap<String, String>,
    body: Vec<u8>,
) -> Response {
    let headers = lower_headers(&headers);
    // 0. Endpoint config + compiled rules from the cache (no DB read on a match).
    let ep = match state.rule_cache.get(&state.pool, token).await {
        Ok(Some(Resolved::Live(ep))) => ep,
        Ok(Some(Resolved::Gone)) => return unknown_or_gone(token, true),
        Ok(None) | Err(_) => return unknown_or_gone(token, false),
    };
    // 0a. Ingest cap backstop (the router enforces it before buffering too).
    if body.len() > state.cfg.max_ingest_body_bytes {
        let resp = MockError::PayloadTooLarge.into_response();
        return identified(resp, token, ServedBy::Default, None, &ep, &headers);
    }
    let req = MockRequest::new(token, method, mock_path, headers, query, body);
    let mut ex = Exchange::new(state, req, ep);

    // 1. OPTIONS preflight short-circuit.
    if ex.req.method == "OPTIONS" {
        let resp = respond::preflight(&ex);
        return ex.finish(resp, ServedBy::Cors).await;
    }
    // 2. Lazy state read; 3. a matching rule; 4. otherwise the fallbacks.
    ex.read_state_if_gated().await;
    let ep = ex.ep.clone();
    let (resp, served_by) = match ex.select_rule(&ep) {
        Some(matched) => (rule::respond(&mut ex, matched).await, ServedBy::Rule),
        None => unmatched::resolve(&mut ex).await,
    };
    // 5. Conditions: rate-limit -> chaos -> latency; 6–7. finish.
    match wrap::apply(&mut ex).await {
        Wrapped::Continue => ex.finish(resp, served_by).await,
        Wrapped::Replace(early, by) => ex.finish(early, by).await,
        Wrapped::Dropout => ex.finish_dropout(),
    }
}
