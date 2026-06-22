//! Live-feed WS + SSE endpoints — PORT of `app/websocket.py` (§5.4, AC-41..44,
//! AC-S9/S13/S19).
//!
//! `GET /ws/{token}?cap=` and `GET /sse/{token}?cap=`: owner-gated via `?cap=`,
//! verified BEFORE any frame. On auth failure: WS accept-then-close `4401` (so the
//! code reaches the client, not a 1006); SSE `401`. Connection cap per endpoint =
//! `WS_MAX_CONN_PER_ENDPOINT`: WS accept-then-close `1013`; SSE `503`. First frame
//! is `hello {token, server_time}`. WS `"ping"` → `"pong"`; SSE `: ping` heartbeat
//! ~25s. A lagged broadcast receiver drops frames (client reconciles via the
//! management API) and never errors the socket. The `cap` is never logged.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, Stream, StreamExt};
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;

use crate::error::ApiError;
use crate::feed::FeedEvent;
use crate::state::AppState;

const SSE_HEARTBEAT_S: u64 = 25;

#[derive(serde::Deserialize)]
pub struct FeedQuery {
    pub cap: Option<String>,
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

// --- WS ----------------------------------------------------------------------

pub async fn ws_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(q): Query<FeedQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let authed = crate::auth::verify_cap_owns_token(&state.pool, &token, q.cap.as_deref()).await;
    let at_cap = state.feed_hub.subscriber_count(&token) as i64 >= state.cfg.ws_max_conn_per_endpoint;
    ws.on_upgrade(move |socket| async move {
        if !authed {
            close_ws(socket, 4401, "unauthorized").await;
            return;
        }
        if at_cap {
            close_ws(socket, 1013, "too many connections").await;
            return;
        }
        run_ws(state, token, socket).await;
    })
}

async fn close_ws(mut socket: WebSocket, code: u16, reason: &str) {
    let _ = socket
        .send(Message::Close(Some(axum::extract::ws::CloseFrame {
            code,
            reason: reason.to_string().into(),
        })))
        .await;
}

async fn run_ws(state: AppState, token: String, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let mut sub = state.feed_hub.subscribe(&token);

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
        }
    }
}

// --- SSE ---------------------------------------------------------------------

pub async fn sse_handler(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Query(q): Query<FeedQuery>,
) -> Response {
    if !crate::auth::verify_cap_owns_token(&state.pool, &token, q.cap.as_deref()).await {
        return ApiError::unauthorized("Valid owner capability required.").into_response();
    }
    if state.feed_hub.subscriber_count(&token) as i64 >= state.cfg.ws_max_conn_per_endpoint {
        return ApiError::new(axum::http::StatusCode::SERVICE_UNAVAILABLE, "too_many_connections", "Too many feed connections.").into_response();
    }
    let sub = state.feed_hub.subscribe(&token);
    let token_for_hello = token.clone();
    let stream = sse_stream(sub, token_for_hello);
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(SSE_HEARTBEAT_S)).text("ping"))
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
            match sub.rx.recv().await {
                Ok(event) => {
                    yield Ok(sse_event(&event));
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    }
}

fn sse_event(event: &FeedEvent) -> Event {
    Event::default().event(&event.kind).data(event.data.to_string())
}
