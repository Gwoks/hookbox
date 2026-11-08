//! WebSocket close codes shared by the live feed, the tunnel control channel
//! and the `tunnel` CLI (which imports them from here instead of redeclaring
//! them), plus the one accept-then-close helper.

use axum::extract::ws::{CloseFrame, Message, WebSocket};

/// Application close: missing/invalid owner capability (feed + tunnel bind).
/// Sent after accept so the code reaches the client instead of a bare 1006.
pub const WS_CLOSE_UNAUTHORIZED: u16 = 4401;

/// Application close: a newer authenticated bind took over this tunnel slug
/// (last-authenticated-bind-wins).
pub const WS_CLOSE_REBOUND: u16 = 4409;

/// RFC 6455 "Try Again Later": the per-endpoint feed connection cap is full.
pub const WS_CLOSE_TRY_AGAIN_LATER: u16 = 1013;

/// RFC 6455 "Service Restart": the owner key was rotated, so the feed drops
/// every socket authenticated before the rotation. Clients reconnect normally;
/// one holding the current key (the rotating browser's refreshed cookie) is
/// readmitted, one holding the old key fails the handshake with 4401.
pub const WS_CLOSE_RECONNECT: u16 = 1012;

/// A close frame carrying `code`/`reason` (for sockets already split into a sink).
pub(crate) fn close_frame(code: u16, reason: &str) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: reason.to_string().into(),
    }))
}

/// Send a single close frame with `code`/`reason`, then drop the socket. A
/// send failure means the peer is already gone, so it is ignored.
pub(crate) async fn close_with(mut socket: WebSocket, code: u16, reason: &str) {
    let _ = socket.send(close_frame(code, reason)).await;
}
