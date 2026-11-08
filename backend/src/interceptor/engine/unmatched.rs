//! No rule matched (§5.5 step 4): Auto-CRUD → tunnel → MITM → default, in
//! that frozen order.

use std::time::Duration;

use axum::body::Body;
use axum::response::{IntoResponse, Response};

use super::echo;
use super::exchange::Exchange;
use super::respond::{insert_header, json_response, with_status};
use crate::interceptor::crud;
use crate::interceptor::mock_error::MockError;
use crate::interceptor::proxy::{self, ProxyError};
use crate::models::{DefaultMode, ServedBy};
use crate::tunnels::TunnelRequest;

pub(super) async fn resolve(ex: &mut Exchange<'_>) -> (Response, ServedBy) {
    if ex.ep.auto_crud && crud::matches(&ex.req.path) {
        return (auto_crud(ex).await, ServedBy::Crud);
    }
    if ex.state.tunnels.is_active(&ex.req.token) {
        return (tunnel(ex).await, ServedBy::Tunnel);
    }
    let target = ex.ep.target_url.clone().filter(|t| !t.trim().is_empty());
    if let Some(target) = target {
        return (mitm(ex, &target).await, ServedBy::Mitm);
    }
    (default_mode(ex), ServedBy::Default)
}

/// 4a. The endpoint as a zero-config REST store.
async fn auto_crud(ex: &mut Exchange<'_>) -> Response {
    let r = &ex.req;
    let outcome = crud::handle(ex.state, &r.token, &r.method, &r.path, &r.body_text).await;
    let detail = format!("{} {}", r.method, r.path);
    match outcome {
        Ok(cr) => {
            ex.step("crud", detail);
            match cr.body {
                Some(body) => json_response(cr.status, &body),
                None => with_status(cr.status, Body::empty()),
            }
        }
        Err(e) => {
            tracing::warn!("auto-crud store error: {e}");
            ex.step("crud", "store error -> 503");
            MockError::CrudStoreUnavailable.into_response()
        }
    }
}

/// 4b. Replay down the bound tunnel CLI.
async fn tunnel(ex: &mut Exchange<'_>) -> Response {
    let r = &ex.req;
    let req = TunnelRequest {
        method: &r.method,
        path: &r.path,
        query: &r.query,
        headers: &r.headers,
        body: &r.body,
    };
    let timeout = Duration::from_secs(ex.state.cfg.tunnel_request_timeout_s);
    let outcome = ex.state.tunnels.forward(&r.token, &req, timeout).await;
    match outcome {
        Ok(resp) => {
            ex.step("tunnel", "forwarded to bound CLI");
            resp
        }
        Err(e) => {
            tracing::debug!("tunnel forward failed: {e}");
            ex.step("tunnel", "no_tunnel");
            MockError::NoTunnel.into_response()
        }
    }
}

/// 4c. MITM forward to the endpoint's `target_url`.
async fn mitm(ex: &mut Exchange<'_>, target: &str) -> Response {
    let r = &ex.req;
    let outcome = proxy::forward(
        &ex.state.cfg,
        target,
        &r.method,
        &r.path,
        &r.query,
        &r.headers,
        &r.body,
    )
    .await;
    match outcome {
        Ok(upstream) => {
            ex.step("forward", format!("MITM -> {target}"));
            let mut resp = with_status(upstream.status, Body::from(upstream.body));
            for (k, v) in &upstream.headers {
                insert_header(resp.headers_mut(), k, v);
            }
            resp
        }
        Err(ProxyError::Timeout) => {
            ex.step("forward", "MITM timeout -> 504");
            MockError::UpstreamTimeout.into_response()
        }
        Err(ProxyError::Unreachable(_)) => {
            ex.step("forward", "MITM unreachable -> 502");
            MockError::UpstreamUnreachable.into_response()
        }
    }
}

/// 4d. `default_mode`: echo the request back, or 404 `no_match`.
fn default_mode(ex: &mut Exchange<'_>) -> Response {
    if ex.ep.default_mode == DefaultMode::Echo {
        ex.step("default", "echo");
        return json_response(200, &echo::client_echo(&ex.req));
    }
    ex.step("default", "mock_404");
    MockError::NoMatch.into_response()
}
