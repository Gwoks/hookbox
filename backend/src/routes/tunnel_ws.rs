//! Tunnel control-channel WS server — PORT of `app/routes/tunnel.py` (§5.8,
//! AC-49..52, AC-S10).
//!
//! `/ws/tunnel/{slug}`: bind auth is `Authorization: Bearer <owner key>` ONLY,
//! verified BEFORE registration (unauth/wrong-owner → close `4401`, never
//! registered). There is no `?cap=` (it lands in proxy access logs) and the
//! session cookie is NEVER accepted: a page on a sibling subdomain (e.g. a
//! wildcard mock host) could otherwise open a socket that the browser decorates
//! with the cookie and bind the owner's tunnel. Bound greeting
//! `{t:bound,slug}`. Frames are JSON multiplexed by integer `id`
//! (req/res/err/ping/pong, base64 bodies). Last-authenticated-bind-wins: a new
//! bind displaces the prior socket (`{t:err,message:"rebound elsewhere"}` then
//! close `4409`). Rotating the owner key revokes the binding (close `4401`).
//! `TunnelRegistry::forward` (tunnels.rs) replays a public request down the
//! bound socket and awaits the matching `res`, bounded by
//! `TUNNEL_REQUEST_TIMEOUT_S` → `504 no_tunnel` (never hangs).

use std::sync::Arc;

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::auth::{key_owns_token, parse_bearer};
use crate::feed::FeedEvent;
use crate::net::ws::{close_with, WS_CLOSE_REBOUND, WS_CLOSE_UNAUTHORIZED};
use crate::state::AppState;
use crate::tunnels::TunnelConnection;

/// `/ws/tunnel/{slug}` handler. Auth runs BEFORE upgrade-accept; on failure we
/// accept-then-close 4401 so the CLI sees the application close code.
pub async fn tunnel_ws_handler(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    ws: WebSocketUpgrade,
    headers: HeaderMap,
) -> Response {
    let bearer = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_bearer);
    let authed = key_owns_token(&state.pool, &slug, bearer).await;

    ws.on_upgrade(move |socket| async move {
        if !authed {
            close_with(socket, WS_CLOSE_UNAUTHORIZED, "unauthorized").await;
            return;
        }
        run_tunnel(state, slug, socket).await;
    })
}

fn close_frame(code: u16, reason: &str) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: reason.to_string().into(),
    }))
}

/// Outbound pump: forward queued frames (req / rebound err) to the socket.
/// Ends with close 4401 when the binding is revoked (owner key rotated), or
/// 4409 when the frame channel closes (takeover).
fn spawn_pump(
    mut sink: SplitSink<WebSocket, Message>,
    mut out_rx: mpsc::UnboundedReceiver<Value>,
    conn: Arc<TunnelConnection>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                frame = out_rx.recv() => match frame {
                    Some(frame) => {
                        if sink.send(Message::Text(frame.to_string())).await.is_err() {
                            return;
                        }
                    }
                    None => break,
                },
                _ = conn.revoked.notified() => {
                    let _ = sink.send(close_frame(WS_CLOSE_UNAUTHORIZED, "owner key rotated")).await;
                    return;
                }
            }
        }
        let _ = sink
            .send(close_frame(WS_CLOSE_REBOUND, "rebound elsewhere"))
            .await;
    })
}

async fn run_tunnel(state: AppState, slug: String, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (out_tx, out_rx) = mpsc::unbounded_channel::<Value>();
    let (conn, generation) = state.tunnels.bind(&slug, out_tx);
    publish_endpoint_updated(&state, &slug);

    // Greet the CLI.
    let _ = sink
        .send(Message::Text(json!({"t":"bound","slug": slug}).to_string()))
        .await;

    let mut pump = spawn_pump(sink, out_rx, conn.clone());

    // Inbound loop: dispatch CLI frames until the CLI leaves or the pump
    // ends (revoked / socket write failed).
    loop {
        let msg = tokio::select! {
            msg = stream.next() => msg,
            _ = &mut pump => break,
        };
        let Some(Ok(msg)) = msg else { break };
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

async fn handle_cli_frame(conn: &TunnelConnection, raw: &str) {
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

/// Tell feed subscribers that `token`'s `tunnel_active` may have changed.
pub(crate) fn publish_endpoint_updated(state: &AppState, token: &str) {
    state.feed_hub.publish(
        token,
        FeedEvent::new(
            "endpoint_updated",
            json!({"token": token, "fields": ["tunnel_active"]}),
        ),
    );
}
