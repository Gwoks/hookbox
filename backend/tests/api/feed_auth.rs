//! Live-feed and tunnel handshake auth (AC-S14c): the feed accepts the owner
//! key as Bearer or session cookie (never `?cap=`) and rejects a foreign
//! Origin with HTTP 403 before any upgrade; the tunnel accepts ONLY a Bearer
//! and is revoked (close 4401) when the owner rotates the key.

use std::net::SocketAddr;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{self, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tower::ServiceExt;

use crate::common::{call, cookie_header, TestApp, APP_HOST};

const WAIT: Duration = Duration::from_secs(5);

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Serve `app` on an ephemeral loopback port (WebSocket upgrades need a
/// real connection; `oneshot` has no upgrade machinery).
async fn serve(app: Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    addr
}

async fn ws_connect(
    addr: SocketAddr,
    path: &str,
    headers: &[(&str, &str)],
) -> Result<Ws, tungstenite::Error> {
    let mut req = format!("ws://{addr}{path}").into_client_request().unwrap();
    for (k, v) in headers {
        let name = axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap();
        req.headers_mut().insert(name, v.parse().unwrap());
    }
    tokio_tungstenite::connect_async(req)
        .await
        .map(|(ws, _)| ws)
}

/// The next frame, bounded by `WAIT`.
async fn next_frame(ws: &mut Ws) -> Message {
    tokio::time::timeout(WAIT, ws.next())
        .await
        .expect("frame within timeout")
        .expect("stream open")
        .expect("frame ok")
}

async fn expect_close(ws: &mut Ws, code: u16) {
    match next_frame(ws).await {
        Message::Close(Some(frame)) => assert_eq!(u16::from(frame.code), code),
        other => panic!("expected close {code}, got {other:?}"),
    }
}

async fn expect_json(ws: &mut Ws) -> Value {
    match next_frame(ws).await {
        Message::Text(t) => serde_json::from_str(&t).unwrap(),
        other => panic!("expected a text frame, got {other:?}"),
    }
}

async fn primary_token(app: &Router, key: &str) -> String {
    let (_, me, _) = call(app, "GET", APP_HOST, "/api/session", Some(key), None).await;
    me["primary"]["token"].as_str().unwrap().to_string()
}

/// Status of an SSE request without reading the (endless) event stream.
async fn sse_status(app: &Router, path: &str, headers: &[(&str, &str)]) -> StatusCode {
    let req = headers
        .iter()
        .fold(
            Request::builder().uri(path).header("host", APP_HOST),
            |b, (k, v)| b.header(*k, *v),
        )
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

// --- SSE -----------------------------------------------------------------------

#[tokio::test]
async fn sse_accepts_cookie_or_bearer_and_rejects_foreign_origin_and_cap() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = primary_token(&app, &secret).await;
    let path = format!("/sse/{token}");
    let (c, cookie) = cookie_header(&secret);
    let bearer = format!("Bearer {secret}");

    let ok_cookie = [(c, cookie.as_str()), ("origin", "http://app.local")];
    assert_eq!(sse_status(&app, &path, &ok_cookie).await, StatusCode::OK);
    let ok_bearer = [("authorization", bearer.as_str())];
    assert_eq!(sse_status(&app, &path, &ok_bearer).await, StatusCode::OK);

    let evil = [(c, cookie.as_str()), ("origin", "https://evil.example")];
    assert_eq!(sse_status(&app, &path, &evil).await, StatusCode::FORBIDDEN);
    let cross = [(c, cookie.as_str()), ("sec-fetch-site", "cross-site")];
    assert_eq!(sse_status(&app, &path, &cross).await, StatusCode::FORBIDDEN);

    let cap_only = format!("{path}?cap={secret}");
    assert_eq!(
        sse_status(&app, &cap_only, &[]).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(sse_status(&app, &path, &[]).await, StatusCode::UNAUTHORIZED);
}

// --- WS feed -------------------------------------------------------------------

#[tokio::test]
async fn ws_feed_with_cookie_says_hello() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = primary_token(&app, &secret).await;
    let addr = serve(app).await;
    let (c, cookie) = cookie_header(&secret);
    let origin = format!("http://{addr}");
    let mut ws = ws_connect(
        addr,
        &format!("/ws/{token}"),
        &[(c, &cookie), ("origin", &origin)],
    )
    .await
    .expect("same-origin cookie handshake");
    let hello = expect_json(&mut ws).await;
    assert_eq!(hello["type"], json!("hello"));
    assert_eq!(hello["data"]["token"], json!(token));
}

#[tokio::test]
async fn ws_feed_foreign_origin_is_http_403_before_upgrade() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = primary_token(&app, &secret).await;
    let addr = serve(app).await;
    let (c, cookie) = cookie_header(&secret);
    let err = ws_connect(
        addr,
        &format!("/ws/{token}"),
        &[(c, &cookie), ("origin", "https://evil.example")],
    )
    .await
    .expect_err("foreign origin must not upgrade");
    match err {
        tungstenite::Error::Http(resp) => assert_eq!(resp.status(), 403),
        other => panic!("expected HTTP 403, got {other:?}"),
    }
}

#[tokio::test]
async fn ws_feed_cap_query_alone_is_closed_4401() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = primary_token(&app, &secret).await;
    let addr = serve(app).await;
    let mut ws = ws_connect(addr, &format!("/ws/{token}?cap={secret}"), &[])
        .await
        .unwrap();
    expect_close(&mut ws, 4401).await;
}

// --- tunnel ----------------------------------------------------------------------

#[tokio::test]
async fn tunnel_rejects_cap_and_cookie_and_binds_with_bearer() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = primary_token(&app, &secret).await;
    let addr = serve(app).await;
    let path = format!("/ws/tunnel/{token}");

    let mut by_cap = ws_connect(addr, &format!("{path}?cap={secret}"), &[])
        .await
        .unwrap();
    expect_close(&mut by_cap, 4401).await;

    let (c, cookie) = cookie_header(&secret);
    let origin = format!("http://{addr}");
    let cookie_headers = [
        (c, cookie.as_str()),
        ("x-hookbox", "1"),
        ("origin", origin.as_str()),
    ];
    let mut by_cookie = ws_connect(addr, &path, &cookie_headers).await.unwrap();
    expect_close(&mut by_cookie, 4401).await;

    let bearer = format!("Bearer {secret}");
    let mut by_bearer = ws_connect(addr, &path, &[("authorization", &bearer)])
        .await
        .unwrap();
    let bound = expect_json(&mut by_bearer).await;
    assert_eq!(bound, json!({"t": "bound", "slug": token}));
}

#[tokio::test]
async fn rotating_the_key_closes_a_bound_tunnel_with_4401() {
    let TestApp { app, state, secret } = TestApp::new().await;
    let token = primary_token(&app, &secret).await;
    let addr = serve(app.clone()).await;
    let bearer = format!("Bearer {secret}");
    let mut tunnel = ws_connect(
        addr,
        &format!("/ws/tunnel/{token}"),
        &[("authorization", &bearer)],
    )
    .await
    .unwrap();
    expect_json(&mut tunnel).await; // bound
    assert!(state.tunnels.is_active(&token));

    let (s, _, _) = call(
        &app,
        "POST",
        APP_HOST,
        "/api/session/rotate",
        Some(&secret),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    expect_close(&mut tunnel, 4401).await;
    assert!(!state.tunnels.is_active(&token));
}

async fn rotate(app: &Router, key: &str) {
    let (s, _, _) = call(
        app,
        "POST",
        APP_HOST,
        "/api/session/rotate",
        Some(key),
        None,
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn rotating_the_key_closes_live_ws_feeds_and_the_old_key_cannot_reconnect() {
    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = primary_token(&app, &secret).await;
    let addr = serve(app.clone()).await;
    let bearer = format!("Bearer {secret}");
    let feed = format!("/ws/{token}");
    let mut ws = ws_connect(addr, &feed, &[("authorization", &bearer)])
        .await
        .unwrap();
    expect_json(&mut ws).await; // hello

    rotate(&app, &secret).await;
    // 1012 = reconnect: a client holding the current key is readmitted…
    expect_close(&mut ws, 1012).await;
    // …but the rotated-out key fails the re-handshake.
    let mut again = ws_connect(addr, &feed, &[("authorization", &bearer)])
        .await
        .unwrap();
    expect_close(&mut again, 4401).await;
}

#[tokio::test]
async fn rotating_the_key_ends_live_sse_streams() {
    use http_body_util::BodyExt;

    let TestApp { app, secret, .. } = TestApp::new().await;
    let token = primary_token(&app, &secret).await;
    let bearer = format!("Bearer {secret}");
    let req = Request::builder()
        .uri(format!("/sse/{token}"))
        .header("host", APP_HOST)
        .header("authorization", &bearer)
        .body(Body::empty())
        .unwrap();
    let mut body = app.clone().oneshot(req).await.unwrap().into_body();
    let hello = tokio::time::timeout(WAIT, body.frame()).await.unwrap();
    assert!(hello.is_some(), "hello event arrives first");

    rotate(&app, &secret).await;
    let ended = tokio::time::timeout(WAIT, async { while body.frame().await.is_some() {} }).await;
    assert!(ended.is_ok(), "the SSE stream ends after rotation");
    let sse = format!("/sse/{token}");
    let old = [("authorization", bearer.as_str())];
    assert_eq!(sse_status(&app, &sse, &old).await, StatusCode::UNAUTHORIZED);
}
