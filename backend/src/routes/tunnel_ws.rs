//! Tunnel control-channel WS server — PORT of `app/routes/tunnel.py` (§5.8,
//! AC-49..52, AC-S10).
//!
//! `/ws/tunnel/{slug}`: bind auth (`Authorization: Bearer` or `?cap=`) is verified
//! BEFORE `accept()`/registration (unauth/wrong-owner → close `4401`, never
//! registered). Bound greeting `{t:bound,slug}`. Frames are JSON multiplexed by
//! integer `id` (req/res/err/ping/pong, base64 bodies). Last-authenticated-bind-
//! wins: a new bind displaces the prior socket (`{t:err,message:"rebound
//! elsewhere"}` then close `4409`). `forward_to_tunnel` replays a public request
//! down the bound socket and awaits the matching `res`, bounded by
//! `TUNNEL_REQUEST_TIMEOUT_S` → `504 no_tunnel` (never hangs).

use std::collections::BTreeMap;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::feed::FeedEvent;
use crate::state::AppState;

pub const WS_CLOSE_UNAUTHORIZED: u16 = 4401;
pub const WS_CLOSE_REBOUND: u16 = 4409;

/// Engine entry point: replay a public request down the bound tunnel (§5.5).
/// Returns the CLI's response, or `Err(())` on no/closed tunnel / timeout / CLI
/// error so the engine surfaces a deterministic `504 no_tunnel`.
pub async fn forward_to_tunnel(
    state: &AppState,
    token: &str,
    method: &str,
    mock_path: &str,
    query: &BTreeMap<String, String>,
    headers: &BTreeMap<String, String>,
    body: &[u8],
) -> Result<Response, ()> {
    let conn = state.tunnels.get(token).ok_or(())?;
    let (id, rx) = conn.register_request().await;
    let frame = json!({
        "t": "req",
        "id": id,
        "method": method,
        "path": mock_path,
        "query": query,
        "headers": headers,
        "body_b64": base64::engine::general_purpose::STANDARD.encode(body),
    });
    if conn.outbound.send(frame).is_err() {
        conn.forget(id).await;
        return Err(());
    }
    let timeout = Duration::from_secs(state.cfg.tunnel_request_timeout_s);
    let res = match tokio::time::timeout(timeout, rx).await {
        Ok(Ok(frame)) => frame,
        _ => {
            conn.forget(id).await;
            return Err(());
        }
    };
    // Build the response from the CLI's res frame.
    let status = res.get("status").and_then(Value::as_i64).unwrap_or(200) as u16;
    let body_b64 = res.get("body_b64").and_then(Value::as_str).unwrap_or("");
    let body_bytes = base64::engine::general_purpose::STANDARD
        .decode(body_b64)
        .unwrap_or_default();
    let mut resp = Response::builder()
        .status(StatusCode::from_u16(status).unwrap_or(StatusCode::OK))
        .body(Body::from(body_bytes))
        .unwrap();
    if let Some(h) = res.get("headers").and_then(Value::as_object) {
        for (k, v) in h {
            let kl = k.to_ascii_lowercase();
            if matches!(
                kl.as_str(),
                "connection"
                    | "keep-alive"
                    | "transfer-encoding"
                    | "content-length"
                    | "content-encoding"
            ) {
                continue;
            }
            if let (Ok(name), Some(val)) = (HeaderName::from_bytes(k.as_bytes()), v.as_str()) {
                if let Ok(hv) = HeaderValue::from_str(val) {
                    resp.headers_mut().insert(name, hv);
                }
            }
        }
    }
    Ok(resp)
}

#[derive(serde::Deserialize)]
pub struct TunnelQuery {
    pub cap: Option<String>,
}

/// `/ws/tunnel/{slug}` handler. Auth runs BEFORE upgrade-accept; on failure we
/// accept-then-close 4401 so the CLI sees the application close code.
pub async fn tunnel_ws_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    Query(q): Query<TunnelQuery>,
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
) -> Response {
    let bearer = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_bearer);
    let secret = bearer.or(q.cap);
    let authed = crate::auth::verify_cap_owns_token(&state.pool, &slug, secret.as_deref()).await;

    ws.on_upgrade(move |socket| async move {
        if !authed {
            close_with(socket, WS_CLOSE_UNAUTHORIZED, "unauthorized").await;
            return;
        }
        run_tunnel(state, slug, socket).await;
    })
}

fn parse_bearer(h: &str) -> Option<String> {
    let mut parts = h.splitn(2, ' ');
    let scheme = parts.next().unwrap_or("");
    let tok = parts.next().unwrap_or("").trim();
    if scheme.eq_ignore_ascii_case("bearer") && !tok.is_empty() {
        Some(tok.to_string())
    } else {
        None
    }
}

async fn close_with(mut socket: WebSocket, code: u16, reason: &str) {
    let _ = socket
        .send(Message::Close(Some(axum::extract::ws::CloseFrame {
            code,
            reason: reason.to_string().into(),
        })))
        .await;
}

async fn run_tunnel(state: AppState, slug: String, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<Value>();
    let (conn, generation) = state.tunnels.bind(&slug, out_tx);
    publish_endpoint_updated(&state, &slug);

    // Greet the CLI.
    let _ = sink
        .send(Message::Text(json!({"t":"bound","slug": slug}).to_string()))
        .await;

    // Outbound pump: forward queued frames (req / rebound err) to the socket.
    let pump = tokio::spawn(async move {
        while let Some(frame) = out_rx.recv().await {
            if sink.send(Message::Text(frame.to_string())).await.is_err() {
                break;
            }
        }
        // On takeover, send a clear close.
        let _ = sink
            .send(Message::Close(Some(axum::extract::ws::CloseFrame {
                code: WS_CLOSE_REBOUND,
                reason: "rebound elsewhere".to_string().into(),
            })))
            .await;
    });

    // Inbound loop: dispatch CLI frames.
    while let Some(Ok(msg)) = stream.next().await {
        // If a newer bind took over, stop serving this socket.
        if state.tunnels.current_generation(&slug) != Some(generation) {
            let _ = conn
                .outbound
                .send(json!({"t":"err","message":"rebound elsewhere"}));
            break;
        }
        match msg {
            Message::Text(raw) => handle_cli_frame(&conn, &raw).await,
            Message::Close(_) => break,
            _ => {}
        }
    }

    pump.abort();
    state.tunnels.unbind_if(&slug, generation);
    publish_endpoint_updated(&state, &slug);
}

async fn handle_cli_frame(conn: &crate::tunnels::TunnelConnection, raw: &str) {
    let frame: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return,
    };
    match frame.get("t").and_then(Value::as_str) {
        Some("res") => {
            if let Some(id) = frame.get("id").and_then(Value::as_u64) {
                conn.resolve(id, frame).await;
            }
        }
        Some("err") => {
            if let Some(id) = frame.get("id").and_then(Value::as_u64) {
                // Resolve with an error marker so the waiter maps to 504.
                conn.forget(id).await;
            }
        }
        Some("ping") => {
            let _ = conn.outbound.send(json!({"t":"pong"}));
        }
        _ => {}
    }
}

fn publish_endpoint_updated(state: &AppState, token: &str) {
    state.feed_hub.publish(
        token,
        FeedEvent::new(
            "endpoint_updated",
            json!({"token": token, "fields": ["tunnel_active"]}),
        ),
    );
}
