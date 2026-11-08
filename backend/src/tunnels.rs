//! In-process tunnel registry — PORT of `app/routes/tunnel.py::TunnelRegistry`
//! (§5.8, AC-49..52, AC-S10).
//!
//! Maps `token -> TunnelConnection`; not in SQLite by design (§5.6). Each
//! connection holds an outbound frame sender (to the bound CLI socket) and a
//! `Mutex`-guarded map of pending request ids → oneshot response channels (R2:
//! the per-request-id oneshot map must be Mutex-guarded under multi-threaded
//! tokio). `bind` is last-authenticated-bind-wins: a new bind for the same token
//! replaces the prior connection (whose socket task then closes 4409).

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::Response;
use base64::Engine;
use dashmap::DashMap;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Mutex, Notify};

use crate::net::headers::{is_tunnel_response_stripped, try_header};

/// One bound tunnel CLI connection.
pub struct TunnelConnection {
    /// Outbound frames (JSON values) to send down the CLI socket.
    pub outbound: mpsc::UnboundedSender<Value>,
    /// Pending request id -> response oneshot (R2: Mutex-guarded).
    pending: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    next_id: AtomicU64,
    /// Bumped each bind; the socket task compares to detect a takeover (4409).
    pub generation: u64,
    /// Fired by `TunnelRegistry::revoke` (owner key rotated): the socket
    /// task closes the CLI connection with 4401.
    pub revoked: Notify,
}

impl TunnelConnection {
    pub fn new(outbound: mpsc::UnboundedSender<Value>, generation: u64) -> Self {
        TunnelConnection {
            outbound,
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            generation,
            revoked: Notify::new(),
        }
    }

    fn alloc_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Register a pending request and return its id + the receiving oneshot.
    pub async fn register_request(&self) -> (u64, oneshot::Receiver<Value>) {
        let id = self.alloc_id();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        (id, rx)
    }

    /// Resolve a pending request with the CLI's response frame.
    pub async fn resolve(&self, id: u64, frame: Value) {
        if let Some(tx) = self.pending.lock().await.remove(&id) {
            let _ = tx.send(frame);
        }
    }

    /// Drop a pending request (timeout / drop).
    pub async fn forget(&self, id: u64) {
        self.pending.lock().await.remove(&id);
    }
}

pub struct TunnelRegistry {
    conns: DashMap<String, Arc<TunnelConnection>>,
    generation: AtomicU64,
}

impl Default for TunnelRegistry {
    fn default() -> Self {
        TunnelRegistry {
            conns: DashMap::new(),
            generation: AtomicU64::new(1),
        }
    }
}

impl TunnelRegistry {
    pub fn new() -> Self {
        TunnelRegistry::default()
    }

    /// Whether a CLI is currently bound for this endpoint token.
    pub fn is_active(&self, token: &str) -> bool {
        self.conns.contains_key(token)
    }

    /// Bind a new connection for `token` (last-authenticated-bind-wins). Returns
    /// `(connection, generation)`; the caller's socket task should exit when it
    /// observes a different current generation (takeover → 4409).
    pub fn bind(
        &self,
        token: &str,
        outbound: mpsc::UnboundedSender<Value>,
    ) -> (Arc<TunnelConnection>, u64) {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst);
        let conn = Arc::new(TunnelConnection::new(outbound, generation));
        self.conns.insert(token.to_string(), conn.clone());
        (conn, generation)
    }

    /// The current generation bound for `token` (None if unbound).
    pub fn current_generation(&self, token: &str) -> Option<u64> {
        self.conns.get(token).map(|c| c.generation)
    }

    /// Unbind a token iff it still holds `generation` (avoid clobbering a newer
    /// bind during teardown).
    pub fn unbind_if(&self, token: &str, generation: u64) {
        if let Some(c) = self.conns.get(token) {
            if c.generation == generation {
                drop(c);
                self.conns.remove(token);
            }
        }
    }

    pub fn get(&self, token: &str) -> Option<Arc<TunnelConnection>> {
        self.conns.get(token).map(|c| c.clone())
    }

    /// Drop whatever tunnel is bound for `token` (owner key rotated):
    /// `is_active` is false immediately and the bound socket is told to
    /// close. Returns whether a tunnel was bound.
    pub fn revoke(&self, token: &str) -> bool {
        match self.conns.remove(token) {
            Some((_, conn)) => {
                // `notify_one` stores a permit, so the socket task sees it
                // even if it is not parked on `notified()` right now.
                conn.revoked.notify_one();
                true
            }
            None => false,
        }
    }
}

/// Why a tunnel replay produced no CLI response (the engine answers every
/// one with a deterministic `504 no_tunnel`; it never hangs).
#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error("no tunnel CLI is bound for this endpoint")]
    NotBound,
    #[error("the tunnel channel closed before the CLI answered")]
    ChannelClosed,
    #[error("the tunnel CLI did not answer within the request timeout")]
    Timeout,
}

/// One public mock request to replay down a bound tunnel.
pub(crate) struct TunnelRequest<'a> {
    pub(crate) method: &'a str,
    pub(crate) path: &'a str,
    pub(crate) query: &'a BTreeMap<String, String>,
    pub(crate) headers: &'a BTreeMap<String, String>,
    pub(crate) body: &'a [u8],
}

impl TunnelRequest<'_> {
    /// The multiplexed `req` frame (base64 body) for request id `id`.
    fn frame(&self, id: u64) -> Value {
        json!({
            "t": "req",
            "id": id,
            "method": self.method,
            "path": self.path,
            "query": self.query,
            "headers": self.headers,
            "body_b64": base64::engine::general_purpose::STANDARD.encode(self.body),
        })
    }
}

impl TunnelRegistry {
    /// Replay a public request down `token`'s bound tunnel and await the
    /// CLI's matching `res` frame, bounded by `timeout` (§5.5, §5.8).
    pub(crate) async fn forward(
        &self,
        token: &str,
        req: &TunnelRequest<'_>,
        timeout: Duration,
    ) -> Result<Response, TunnelError> {
        let conn = self.get(token).ok_or(TunnelError::NotBound)?;
        let (id, rx) = conn.register_request().await;
        if conn.outbound.send(req.frame(id)).is_err() {
            conn.forget(id).await;
            return Err(TunnelError::ChannelClosed);
        }
        let outcome = match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(frame)) => return Ok(response_from_frame(&frame)),
            Ok(Err(_)) => TunnelError::ChannelClosed,
            Err(_) => TunnelError::Timeout,
        };
        conn.forget(id).await;
        Err(outcome)
    }
}

/// Build the mock response from the CLI's `res` frame. Hop-by-hop / framing
/// headers the CLI echoes back are dropped (`is_tunnel_response_stripped`).
fn response_from_frame(res: &Value) -> Response {
    // `as u16` (wrapping) matches the CLI contract's historical behavior.
    let status = res.get("status").and_then(Value::as_i64).unwrap_or(200) as u16;
    let body_b64 = res.get("body_b64").and_then(Value::as_str).unwrap_or("");
    let body = base64::engine::general_purpose::STANDARD
        .decode(body_b64)
        .unwrap_or_default();
    let mut resp = Response::new(Body::from(body));
    *resp.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
    let headers = res.get("headers").and_then(Value::as_object);
    for (k, v) in headers.into_iter().flatten() {
        if is_tunnel_response_stripped(k) {
            continue;
        }
        if let Some((name, value)) = v.as_str().and_then(|val| try_header(k, val)) {
            resp.headers_mut().insert(name, value);
        }
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bind_register_resolve() {
        let reg = TunnelRegistry::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let (conn, gen) = reg.bind("tok", tx);
        assert!(reg.is_active("tok"));
        assert_eq!(reg.current_generation("tok"), Some(gen));
        let (id, rx) = conn.register_request().await;
        conn.resolve(id, json!({"t":"res","id":id,"status":200}))
            .await;
        let frame = rx.await.unwrap();
        assert_eq!(frame["status"], json!(200));
    }

    #[tokio::test]
    async fn revoke_unbinds_and_signals_the_socket() {
        let reg = TunnelRegistry::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let (conn, _gen) = reg.bind("tok", tx);
        assert!(reg.revoke("tok"));
        assert!(!reg.is_active("tok"));
        // The stored permit resolves immediately.
        tokio::time::timeout(std::time::Duration::from_secs(1), conn.revoked.notified())
            .await
            .expect("revoke notifies the bound connection");
        assert!(!reg.revoke("tok"), "nothing left to revoke");
    }

    #[tokio::test]
    async fn last_bind_wins() {
        let reg = TunnelRegistry::new();
        let (tx1, _r1) = mpsc::unbounded_channel();
        let (_c1, g1) = reg.bind("tok", tx1);
        let (tx2, _r2) = mpsc::unbounded_channel();
        let (_c2, g2) = reg.bind("tok", tx2);
        assert!(g2 > g1);
        assert_eq!(reg.current_generation("tok"), Some(g2));
        // unbind with the stale generation does nothing.
        reg.unbind_if("tok", g1);
        assert!(reg.is_active("tok"));
        reg.unbind_if("tok", g2);
        assert!(!reg.is_active("tok"));
    }
}
