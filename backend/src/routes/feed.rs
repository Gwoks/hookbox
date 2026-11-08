//! Live-feed WS + SSE endpoints — PORT of `app/websocket.py` (§5.4, AC-41..44,
//! AC-S9/S13/S19).
//!
//! `GET /ws/{token}` and `GET /sse/{token}`: owner-gated by the owner key —
//! `Authorization: Bearer` (CLI/scripts) or the session cookie (the SPA;
//! WebSocket/EventSource cannot set headers, so no `x-hookbox` is required).
//! There is no `?cap=`: a key in the query string lands in proxy access logs.
//! Order: a foreign `Origin`/`Sec-Fetch-Site` is a plain HTTP `403
//! csrf_rejected` BEFORE any upgrade (cross-site WebSocket hijacking guard);
//! then the key is verified BEFORE any frame. On key failure: WS
//! accept-then-close `4401` (so the code reaches the client, not a 1006); SSE
//! `401`. Connection cap per endpoint = `WS_MAX_CONN_PER_ENDPOINT`: WS
//! accept-then-close `1013`; SSE `503`. First frame is `hello {token,
//! server_time}`. WS `"ping"` → `"pong"`; SSE `: ping` heartbeat ~25s. A lagged
//! broadcast receiver drops frames (client reconciles via the management API)
//! and never errors the socket. The key is never logged.
//!
//! Key rotation: `POST /api/session/rotate` revokes the owner's feed channels;
//! live WS sockets then close with `1012` (reconnect) and SSE streams end, so
//! every client must re-handshake with its CURRENT key. Each handler also
//! re-verifies the key right AFTER subscribing (subscribe-then-verify): rotate
//! commits the new key hash before revoking, so a handshake racing a rotation
//! is either revoked or fails this second check — never left streaming.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, Stream, StreamExt};
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;

use crate::auth::{feed_key, key_owns_token};
use crate::error::ApiError;
use crate::feed::FeedEvent;
use crate::net::ws::{
    close_frame, close_with, WS_CLOSE_RECONNECT, WS_CLOSE_TRY_AGAIN_LATER, WS_CLOSE_UNAUTHORIZED,
};
use crate::session::origin_allowed;
use crate::state::AppState;
use crate::util::time::now_iso;

const SSE_HEARTBEAT_S: u64 = 25;

// --- WS ----------------------------------------------------------------------

pub async fn ws_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !origin_allowed(&headers, &state.cfg) {
        return ApiError::csrf_rejected().into_response();
    }
    let key = feed_key(&headers, &state.cfg);
    let authed = key_owns_token(&state.pool, &token, key.as_deref()).await;
    let at_cap =
        state.feed_hub.subscriber_count(&token) as i64 >= state.cfg.ws_max_conn_per_endpoint;
    ws.on_upgrade(move |socket| async move {
        if !authed {
            close_with(socket, WS_CLOSE_UNAUTHORIZED, "unauthorized").await;
            return;
        }
        if at_cap {
            close_with(socket, WS_CLOSE_TRY_AGAIN_LATER, "too many connections").await;
            return;
        }
        run_ws(state, token, key, socket).await;
    })
}

async fn run_ws(state: AppState, token: String, key: Option<String>, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let mut sub = state.feed_hub.subscribe(&token);
    // Subscribe-then-verify: closes the race with a concurrent key rotation.
    if !key_owns_token(&state.pool, &token, key.as_deref()).await {
        let _ = sink
            .send(close_frame(WS_CLOSE_UNAUTHORIZED, "unauthorized"))
            .await;
        return;
    }

    // hello first frame.
    let hello = json!({"type":"hello","data":{"token": token, "server_time": now_iso()}});
    if sink.send(Message::Text(hello.to_string())).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            // Inbound: "ping" -> "pong"; close -> exit.
            inbound = stream.next() => {
                match inbound {
                    Some(Ok(Message::Text(t))) if t == "ping" => {
                        if sink.send(Message::Text("pong".into())).await.is_err() { break; }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            // Outbound: broadcast events. On lag, drop + continue (client reconciles).
            ev = sub.rx.recv() => {
                match ev {
                    Ok(event) => {
                        let frame = event.ws_envelope().to_string();
                        let send = sink.send(Message::Text(frame));
                        // Bound the per-send so a slow client never stalls fan-out.
                        match tokio::time::timeout(
                            Duration::from_secs_f64(state.cfg.ws_send_timeout_s.max(0.1)), send).await {
                            Ok(Ok(())) => {}
                            _ => break, // slow/dead client -> drop
                        }
                    }
                    Err(RecvError::Lagged(_)) => continue, // drop missed frames, client reconciles
                    Err(RecvError::Closed) => break,
                }
            }
            // Owner key rotated: force a re-handshake with the current key.
            _ = sub.epoch.changed() => {
                let _ = sink.send(close_frame(WS_CLOSE_RECONNECT, "owner key rotated")).await;
                break;
            }
        }
    }
}

// --- SSE ---------------------------------------------------------------------

pub async fn sse_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !origin_allowed(&headers, &state.cfg) {
        return ApiError::csrf_rejected().into_response();
    }
    let key = feed_key(&headers, &state.cfg);
    if !key_owns_token(&state.pool, &token, key.as_deref()).await {
        return ApiError::unauthorized("Valid owner capability required.").into_response();
    }
    if state.feed_hub.subscriber_count(&token) as i64 >= state.cfg.ws_max_conn_per_endpoint {
        return ApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            crate::error::ErrorCode::TooManyConnections,
            "Too many feed connections.",
        )
        .into_response();
    }
    let sub = state.feed_hub.subscribe(&token);
    // Subscribe-then-verify (see module docs).
    if !key_owns_token(&state.pool, &token, key.as_deref()).await {
        return ApiError::unauthorized("Valid owner capability required.").into_response();
    }
    let token_for_hello = token.clone();
    let stream = sse_stream(sub, token_for_hello);
    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(SSE_HEARTBEAT_S))
                .text("ping"),
        )
        .into_response()
}

fn sse_stream(
    mut sub: crate::feed::Subscription,
    token: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        // hello first event.
        yield Ok(Event::default().event("hello").data(json!({"token": token, "server_time": now_iso()}).to_string()));
        loop {
            tokio::select! {
                ev = sub.rx.recv() => match ev {
                    Ok(event) => yield Ok(sse_event(&event)),
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                },
                // Owner key rotated: end the stream so EventSource reconnects
                // with the current cookie (the old key then gets a 401).
                _ = sub.epoch.changed() => break,
            }
        }
    }
}

fn sse_event(event: &FeedEvent) -> Event {
    Event::default()
        .event(&event.kind)
        .data(event.data.to_string())
}
