//! In-memory token-bucket rate limiter — PORT of `redis_state.rate_limit_check`
//! + the reference proportional-refill math (AC-39, AC-15, AC-11, AC-S18).
//!
//! Keyed `rl:<token>` / `rl:<token>:<rule_id>` in a `DashMap`. A bucket holds a
//! fractional token count refilled proportionally to elapsed time at
//! `limit/window` tokens/sec; each request consumes one token. Over-limit yields
//! a `retry_after`. `limit<=0` ⇒ unlimited. The limiter **fails open** on any
//! internal anomaly (returns allowed) so a limiter bug never wedges the mock
//! path (it stays bounded by the ingest body cap). The map is **bounded**: idle
//! buckets are evicted by a size cap so it cannot grow without limit.

use std::time::Instant;

use dashmap::DashMap;

/// The result of a rate-limit check.
#[derive(Debug, Clone, PartialEq)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub limit: i64,
    pub remaining: i64,
    pub retry_after: i64,
}

impl RateLimitResult {
    fn unlimited() -> Self {
        RateLimitResult {
            allowed: true,
            limit: 0,
            remaining: -1,
            retry_after: 0,
        }
    }
}

struct Bucket {
    tokens: f64,
    last_refill: Instant,
    last_seen: Instant,
}

/// Hard cap on tracked buckets (bounded map; oldest-idle evicted past this).
const MAX_BUCKETS: usize = 100_000;

pub struct Limiter {
    buckets: DashMap<String, Bucket>,
}

impl Default for Limiter {
    fn default() -> Self {
        Limiter {
            buckets: DashMap::new(),
        }
    }
}

impl Limiter {
    pub fn new() -> Self {
        Limiter::default()
    }

    pub fn len(&self) -> usize {
        self.buckets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    /// Build the bucket key for an endpoint (+ optional per-rule override).
    pub fn key(token: &str, rule_id: Option<i64>) -> String {
        match rule_id {
            Some(id) => format!("rl:{token}:{id}"),
            None => format!("rl:{token}"),
        }
    }

    /// Check + consume one token for `key` against `limit` per `window_secs`.
    /// `limit<=0` ⇒ unlimited. Fails open on any anomaly.
    pub fn check(&self, key: &str, limit: i64, window_secs: i64) -> RateLimitResult {
        if limit <= 0 || window_secs <= 0 {
            return RateLimitResult::unlimited();
        }
        let now = Instant::now();
        let refill_per_sec = limit as f64 / window_secs as f64;

        // Opportunistic bound: if the map is huge, evict the most-idle entry.
        if self.buckets.len() >= MAX_BUCKETS {
            self.evict_one_idle(now);
        }

        let mut entry = self
            .buckets
            .entry(key.to_string())
            .or_insert_with(|| Bucket {
                tokens: limit as f64,
                last_refill: now,
                last_seen: now,
            });

        // Proportional refill since last_refill, capped at `limit`.
        let elapsed = now
            .saturating_duration_since(entry.last_refill)
            .as_secs_f64();
        entry.tokens = (entry.tokens + elapsed * refill_per_sec).min(limit as f64);
        entry.last_refill = now;
        entry.last_seen = now;

        if entry.tokens >= 1.0 {
            entry.tokens -= 1.0;
            RateLimitResult {
                allowed: true,
                limit,
                remaining: entry.tokens.floor() as i64,
                retry_after: 0,
            }
        } else {
            // Seconds until one token is available again.
            let deficit = 1.0 - entry.tokens;
            let retry_after = (deficit / refill_per_sec).ceil() as i64;
            RateLimitResult {
                allowed: false,
                limit,
                remaining: 0,
                retry_after: retry_after.max(1),
            }
        }
    }

    fn evict_one_idle(&self, now: Instant) {
        let mut oldest_key: Option<String> = None;
        let mut oldest = now;
        for kv in self.buckets.iter() {
            if kv.value().last_seen <= oldest {
                oldest = kv.value().last_seen;
                oldest_key = Some(kv.key().clone());
            }
        }
        if let Some(k) = oldest_key {
            self.buckets.remove(&k);
        }
    }

    /// Drop buckets idle longer than `idle_secs` (called by the retention sweep).
    pub fn evict_idle(&self, idle_secs: u64) {
        let now = Instant::now();
        let stale: Vec<String> = self
            .buckets
            .iter()
            .filter(|kv| {
                now.saturating_duration_since(kv.value().last_seen)
                    .as_secs()
                    >= idle_secs
            })
            .map(|kv| kv.key().clone())
            .collect();
        for k in stale {
            self.buckets.remove(&k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlimited_when_limit_zero_or_negative() {
        let l = Limiter::new();
        assert!(l.check("rl:tok", 0, 60).allowed);
        assert!(l.check("rl:tok", -5, 60).allowed);
    }

    #[test]
    fn over_limit_yields_429_with_retry_after() {
        let l = Limiter::new();
        // limit 3/min -> 4th request in the same instant is blocked.
        for _ in 0..3 {
            assert!(l.check("rl:tok", 3, 60).allowed);
        }
        let r = l.check("rl:tok", 3, 60);
        assert!(!r.allowed);
        assert_eq!(r.remaining, 0);
        assert_eq!(r.limit, 3);
        assert!(r.retry_after >= 1);
    }

    #[test]
    fn keys_isolate_endpoint_and_rule() {
        assert_eq!(Limiter::key("tok", None), "rl:tok");
        assert_eq!(Limiter::key("tok", Some(7)), "rl:tok:7");
        let l = Limiter::new();
        // exhaust endpoint bucket; rule bucket is independent.
        for _ in 0..2 {
            l.check("rl:tok", 2, 60);
        }
        assert!(!l.check("rl:tok", 2, 60).allowed);
        assert!(l.check("rl:tok:7", 2, 60).allowed);
    }

    #[test]
    fn idle_eviction_bounds_map() {
        let l = Limiter::new();
        l.check("rl:a", 5, 60);
        l.check("rl:b", 5, 60);
        assert_eq!(l.len(), 2);
        l.evict_idle(0); // everything older than 0s is stale
        assert_eq!(l.len(), 0);
    }
}
