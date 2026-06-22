//! Live-feed broadcast hub — PORT of `app/websocket.py` pub/sub (AC-41..44, §5.4).
//!
//! Per token a `tokio::sync::broadcast::Sender<FeedEvent>` (created lazily on
//! first subscribe) plus an atomic subscriber count for the connection cap.
//! `publish` is non-blocking and a no-op when there are zero receivers — a dead
//! feed never affects the mock path (best-effort/at-most-once; the management API
//! is the source of truth). On a lagged receiver the subscriber drops frames and
//! reconciles via the API rather than erroring the socket.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use serde_json::Value;
use tokio::sync::broadcast;

/// A feed event in the §5.4 `{type, data}` envelope. `kind` is the event name
/// (`new_request` / `state_changed` / `endpoint_updated`); `data` is the payload.
#[derive(Clone, Debug)]
pub struct FeedEvent {
    pub kind: String,
    pub data: Value,
}

impl FeedEvent {
    pub fn new(kind: impl Into<String>, data: Value) -> Self {
        FeedEvent {
            kind: kind.into(),
            data,
        }
    }

    /// The WS wire envelope: `{"type": kind, "data": data}`.
    pub fn ws_envelope(&self) -> Value {
        serde_json::json!({ "type": self.kind, "data": self.data })
    }
}

/// Broadcast channel capacity per token (bounds memory; lag drops on overflow).
const CHANNEL_CAP: usize = 256;

struct Channel {
    tx: broadcast::Sender<FeedEvent>,
    subscribers: AtomicUsize,
}

pub struct FeedHub {
    channels: DashMap<String, Arc<Channel>>,
}

impl Default for FeedHub {
    fn default() -> Self {
        FeedHub {
            channels: DashMap::new(),
        }
    }
}

impl FeedHub {
    pub fn new() -> Self {
        FeedHub::default()
    }

    pub fn len(&self) -> usize {
        self.channels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    fn channel(&self, token: &str) -> Arc<Channel> {
        self.channels
            .entry(token.to_string())
            .or_insert_with(|| {
                let (tx, _rx) = broadcast::channel(CHANNEL_CAP);
                Arc::new(Channel {
                    tx,
                    subscribers: AtomicUsize::new(0),
                })
            })
            .clone()
    }

    /// Current subscriber count for a token (for the connection cap).
    pub fn subscriber_count(&self, token: &str) -> usize {
        self.channels
            .get(token)
            .map(|c| c.subscribers.load(Ordering::SeqCst))
            .unwrap_or(0)
    }

    /// Subscribe to a token's channel. Increments the subscriber count; the
    /// returned guard decrements it on drop and yields a `broadcast::Receiver`.
    pub fn subscribe(&self, token: &str) -> Subscription {
        let ch = self.channel(token);
        ch.subscribers.fetch_add(1, Ordering::SeqCst);
        let rx = ch.tx.subscribe();
        Subscription { ch, rx }
    }

    /// Publish an event for a token. Non-blocking; a no-op when there are zero
    /// receivers (never affects the mock path).
    pub fn publish(&self, token: &str, event: FeedEvent) {
        if let Some(ch) = self.channels.get(token) {
            // `send` errors only when there are no receivers — ignore (no-op).
            let _ = ch.tx.send(event);
        }
    }
}

/// A live subscription. Holds the channel alive and decrements the subscriber
/// count when dropped.
pub struct Subscription {
    ch: Arc<Channel>,
    pub rx: broadcast::Receiver<FeedEvent>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.ch.subscribers.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn publish_reaches_subscribers_and_isolates_tokens() {
        let hub = FeedHub::new();
        let mut sub_a = hub.subscribe("tokA");
        let mut sub_b = hub.subscribe("tokB");
        assert_eq!(hub.subscriber_count("tokA"), 1);

        hub.publish("tokA", FeedEvent::new("new_request", json!({"id": 1})));
        let ev = sub_a.rx.recv().await.unwrap();
        assert_eq!(ev.kind, "new_request");
        assert_eq!(ev.ws_envelope()["type"], json!("new_request"));
        // tokB must not receive tokA's event.
        assert!(sub_b.rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn publish_with_no_receivers_is_noop() {
        let hub = FeedHub::new();
        // never panics / errors even with no channel.
        hub.publish("ghost", FeedEvent::new("new_request", json!({})));
        assert_eq!(hub.subscriber_count("ghost"), 0);
    }

    #[tokio::test]
    async fn subscriber_count_decrements_on_drop() {
        let hub = FeedHub::new();
        {
            let _s = hub.subscribe("t");
            assert_eq!(hub.subscriber_count("t"), 1);
        }
        assert_eq!(hub.subscriber_count("t"), 0);
    }
}
