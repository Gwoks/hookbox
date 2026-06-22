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
    // `Arc` so a `Subscription` can hold a handle and evict its own channel on
    // the last drop — bounding the map to the set of *currently-subscribed*
    // tokens rather than every token ever subscribed (memory bound, AC-S19/N3).
    channels: Arc<DashMap<String, Arc<Channel>>>,
}

impl Default for FeedHub {
    fn default() -> Self {
        FeedHub {
            channels: Arc::new(DashMap::new()),
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

    /// Current subscriber count for a token (for the connection cap).
    pub fn subscriber_count(&self, token: &str) -> usize {
        self.channels
            .get(token)
            .map(|c| c.subscribers.load(Ordering::SeqCst))
            .unwrap_or(0)
    }

    /// Subscribe to a token's channel. Increments the subscriber count; the
    /// returned guard decrements it on drop (and evicts the channel when it
    /// reaches zero) and yields a `broadcast::Receiver`.
    pub fn subscribe(&self, token: &str) -> Subscription {
        // Increment WHILE holding the entry (shard) lock so a concurrent
        // last-unsubscribe eviction (`remove_if`, which checks the count under
        // the same lock) can never drop the channel between lookup and bump.
        let ch = {
            let entry = self.channels.entry(token.to_string()).or_insert_with(|| {
                let (tx, _rx) = broadcast::channel(CHANNEL_CAP);
                Arc::new(Channel {
                    tx,
                    subscribers: AtomicUsize::new(0),
                })
            });
            entry.value().subscribers.fetch_add(1, Ordering::SeqCst);
            entry.value().clone()
        };
        let rx = ch.tx.subscribe();
        Subscription {
            channels: self.channels.clone(),
            token: token.to_string(),
            ch,
            rx,
        }
    }

    /// Publish an event for a token. Non-blocking; a no-op when there are zero
    /// receivers / no channel (never affects the mock path).
    pub fn publish(&self, token: &str, event: FeedEvent) {
        if let Some(ch) = self.channels.get(token) {
            // `send` errors only when there are no receivers — ignore (no-op).
            let _ = ch.tx.send(event);
        }
    }
}

/// A live subscription. Holds the channel alive and decrements the subscriber
/// count when dropped, evicting the channel from the hub on the last drop.
pub struct Subscription {
    channels: Arc<DashMap<String, Arc<Channel>>>,
    token: String,
    ch: Arc<Channel>,
    pub rx: broadcast::Receiver<FeedEvent>,
}

impl Drop for Subscription {
    fn drop(&mut self) {
        // fetch_sub returns the PREVIOUS value; `1` means we were the last.
        if self.ch.subscribers.fetch_sub(1, Ordering::SeqCst) == 1 {
            // Evict iff still at zero, checked under the shard lock so a
            // concurrent `subscribe` (which bumps under the same lock) wins.
            self.channels.remove_if(&self.token, |_, ch| {
                ch.subscribers.load(Ordering::SeqCst) == 0
            });
        }
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

    #[tokio::test]
    async fn channel_is_evicted_when_last_subscriber_drops() {
        let hub = FeedHub::new();
        let s1 = hub.subscribe("t");
        let s2 = hub.subscribe("t");
        assert_eq!(hub.len(), 1);
        drop(s1);
        // Still one subscriber → channel retained.
        assert_eq!(hub.len(), 1);
        drop(s2);
        // Last subscriber gone → channel evicted (map bounded by live subs).
        assert_eq!(hub.len(), 0);
        assert!(hub.is_empty());
    }
}
