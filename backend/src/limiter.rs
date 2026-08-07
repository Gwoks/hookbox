//! In-memory token-bucket rate limiter — PORT of `redis_state.rate_limit_check`
//! + the reference proportional-refill math (AC-39, AC-15, AC-11, AC-S18).
//!
//! Keyed `rl:<token>` / `rl:<token>:<rule_id>` (mock plane, `interceptor::engine`)
//! or `share:<ip>` / `share:__global__` (public share resolver, `routes::share`).
//! A bucket holds a fractional token count refilled proportionally to elapsed
//! time at `limit/window` tokens/sec; each request consumes one token.
//! Over-limit yields a `retry_after`. `limit<=0` ⇒ unlimited. The limiter
//! **fails open** on any internal anomaly (returns allowed) so a limiter bug
//! never wedges the mock path (it stays bounded by the ingest body cap).
//!
//! **Namespaced AND separately bounded (AC-S7).** Buckets are partitioned by
//! namespace — the key segment before the first `:` (`rl` or `share`) — into
//! independent maps, each with its own size cap and its own eviction. An
//! unauthenticated caller flooding the public `share:<ip>` namespace with
//! fresh keys can therefore never evict — or even observe — a `rl:<token>`
//! bucket in the mock-plane namespace: they are structurally different maps.
//! Each namespace's map is bounded: once it hits its cap, the oldest overflow
//! is trimmed off *within that same namespace* in one batch (see
//! `trim_namespace`), so it cannot grow without limit and per-request latency
//! stays stable regardless of how long a flood continues.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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

/// Hard cap on tracked buckets, **per namespace**. Each namespace (`rl` or
/// `share`) gets its own independent budget — the same limit the original
/// single flat map used to enforce globally, it just no longer lets one
/// namespace's churn eat another namespace's budget (AC-S7).
const MAX_BUCKETS_PER_NAMESPACE: usize = 100_000;

/// Once a namespace hits its cap, `trim_namespace` drops it back down to
/// this fraction of the cap in a single batch, evicting the oldest overflow
/// by `last_seen`. Trimming ONE entry per call (the original design) means
/// that once a namespace is pinned at the cap, *every single subsequent
/// call* pays a full scan to find the one most-idle entry — exactly what the
/// bug report measured at 225s for a 101 000-key spray against an
/// already-full map. Trimming a whole batch at once instead means the
/// O(n) scan only runs once per `MAX_BUCKETS_PER_NAMESPACE * (1 -
/// TRIM_TARGET_RATIO)` new keys: the cost is bounded by the (fixed) cap —
/// never by how long a flood continues — and is amortized across thousands
/// of cheap calls in between, which is what keeps per-request latency
/// stable (AC-S7).
const TRIM_TARGET_RATIO: f64 = 0.9;

/// The namespace a bucket key belongs to: the segment before the first `:`
/// (`rl` for `rl:<token>` / `rl:<token>:<rule_id>`, `share` for `share:<ip>` /
/// `share:__global__`). Keys with no `:` are their own one-key namespace.
fn namespace_of(key: &str) -> &str {
    key.split(':').next().unwrap_or(key)
}

/// Drop `map` back down to `TRIM_TARGET_RATIO` of `MAX_BUCKETS_PER_NAMESPACE`
/// in one pass, evicting the oldest entries by `last_seen`. Callers debounce
/// this so only one trim runs at a time per namespace (see `Namespace`).
fn trim_namespace(map: &DashMap<String, Bucket>) {
    let target = ((MAX_BUCKETS_PER_NAMESPACE as f64) * TRIM_TARGET_RATIO) as usize;
    let len = map.len();
    if len <= target {
        return;
    }
    let overflow = len - target;
    let mut entries: Vec<(String, Instant)> = map
        .iter()
        .map(|kv| (kv.key().clone(), kv.value().last_seen))
        .collect();
    entries.sort_unstable_by_key(|(_, last_seen)| *last_seen);
    for (k, _) in entries.into_iter().take(overflow) {
        map.remove(&k);
    }
}

/// One namespace's bucket map plus a debounce flag so concurrent callers
/// that all observe the map at/over cap don't all pay for a redundant batch
/// trim at the same time — only one wins the flag and does the work.
struct Namespace {
    buckets: DashMap<String, Bucket>,
    trimming: AtomicBool,
}

impl Namespace {
    fn new() -> Self {
        Namespace {
            buckets: DashMap::new(),
            trimming: AtomicBool::new(false),
        }
    }
}

pub struct Limiter {
    /// namespace → that namespace's own bounded bucket map. Partitioning by
    /// namespace (rather than one flat map) is what makes AC-S7 structural
    /// rather than best-effort: the public `share:<ip>` plane and the
    /// mock-plane `rl:<token>` buckets simply live in different maps, so one
    /// can never evict — or even be scanned alongside — the other. The
    /// namespace is `Arc`-wrapped so `check()` only needs a short-lived
    /// lookup on the (tiny, effectively-static after warm-up) outer map to
    /// fetch a handle, then does its real work against the inner map with
    /// that map's own per-key shard concurrency.
    namespaces: DashMap<String, Arc<Namespace>>,
}

impl Default for Limiter {
    fn default() -> Self {
        Limiter {
            namespaces: DashMap::new(),
        }
    }
}

impl Limiter {
    pub fn new() -> Self {
        Limiter::default()
    }

    pub fn len(&self) -> usize {
        self.namespaces
            .iter()
            .map(|ns| ns.value().buckets.len())
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.namespaces
            .iter()
            .all(|ns| ns.value().buckets.is_empty())
    }

    /// Build the bucket key for an endpoint (+ optional per-rule override).
    pub fn key(token: &str, rule_id: Option<i64>) -> String {
        match rule_id {
            Some(id) => format!("rl:{token}:{id}"),
            None => format!("rl:{token}"),
        }
    }

    /// Fetch (creating if absent) the namespace for `key`.
    fn namespace(&self, key: &str) -> Arc<Namespace> {
        let ns = namespace_of(key);
        // Fast path: a shared read on the outer map, no exclusive lock — the
        // set of namespaces is small and effectively fixed after warm-up.
        if let Some(existing) = self.namespaces.get(ns) {
            return existing.clone();
        }
        self.namespaces
            .entry(ns.to_string())
            .or_insert_with(|| Arc::new(Namespace::new()))
            .clone()
    }

    /// Check + consume one token for `key` against `limit` per `window_secs`.
    /// `limit<=0` ⇒ unlimited. Fails open on any anomaly.
    pub fn check(&self, key: &str, limit: i64, window_secs: i64) -> RateLimitResult {
        if limit <= 0 || window_secs <= 0 {
            return RateLimitResult::unlimited();
        }
        let now = Instant::now();
        let refill_per_sec = limit as f64 / window_secs as f64;
        let ns = self.namespace(key);

        // Opportunistic bound: if THIS namespace is at capacity, trim it —
        // never another namespace. Debounced so only one caller per
        // namespace pays for the batch trim at a time.
        if ns.buckets.len() >= MAX_BUCKETS_PER_NAMESPACE
            && !ns.trimming.swap(true, Ordering::AcqRel)
        {
            trim_namespace(&ns.buckets);
            ns.trimming.store(false, Ordering::Release);
        }

        let mut entry = ns.buckets.entry(key.to_string()).or_insert_with(|| Bucket {
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

    /// Drop buckets idle longer than `idle_secs` (called by the retention
    /// sweep), namespace by namespace.
    pub fn evict_idle(&self, idle_secs: u64) {
        let now = Instant::now();
        for ns in self.namespaces.iter() {
            let map = &ns.value().buckets;
            let stale: Vec<String> = map
                .iter()
                .filter(|kv| {
                    now.saturating_duration_since(kv.value().last_seen)
                        .as_secs()
                        >= idle_secs
                })
                .map(|kv| kv.key().clone())
                .collect();
            for k in stale {
                map.remove(&k);
            }
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

    /// AC-S7 (MUST) regression: the public share resolver's per-IP keys must
    /// never evict — or reset — a mock-plane `rl:<token>` bucket, no matter
    /// how many distinct `share:<ip>` keys a scanner sprays. Reproduces the
    /// bug's repro at unit-test speed: exhaust a real endpoint's bucket, then
    /// flood the `share` namespace with more distinct keys than the
    /// per-namespace cap, then assert the `rl:<token>` bucket is untouched.
    #[test]
    fn share_namespace_flood_never_evicts_or_resets_rl_bucket() {
        let l = Limiter::new();

        // A real mock-plane bucket: limit 1/min, exhausted by its 2nd hit.
        assert!(l.check("rl:realtoken", 1, 60).allowed);
        let exhausted = l.check("rl:realtoken", 1, 60);
        assert!(!exhausted.allowed);

        // Spray far more distinct public IPs than the per-namespace cap —
        // every one of these creates a brand-new `share:<ip>` bucket.
        for i in 0..(MAX_BUCKETS_PER_NAMESPACE + 1_000) {
            l.check(&format!("share:{i}"), 120, 60);
        }

        // The `share` namespace stayed within its own bound...
        let share_bucket_count = l
            .namespaces
            .get("share")
            .map(|ns| ns.value().buckets.len())
            .unwrap_or(0);
        assert!(
            share_bucket_count <= MAX_BUCKETS_PER_NAMESPACE,
            "share namespace must stay bounded, got {share_bucket_count}"
        );

        // ...and the mock-plane bucket was never touched: still present and
        // still exhausted (not evicted-and-recreated-full, AC-S7's exact
        // attack). If it had been evicted, this check would silently create
        // a FRESH bucket seeded with a full `limit` of tokens and allow.
        let r = l.check("rl:realtoken", 1, 60);
        assert!(
            !r.allowed,
            "rl:<token> bucket must not be reset by a share:<ip> flood"
        );
    }

    /// AC-S7: a `rl:<token>` flood must likewise never touch the `share`
    /// namespace's buckets (the isolation is bidirectional).
    #[test]
    fn rl_namespace_flood_never_touches_share_bucket() {
        let l = Limiter::new();
        assert!(l.check("share:1.2.3.4", 1, 60).allowed);
        let exhausted = l.check("share:1.2.3.4", 1, 60);
        assert!(!exhausted.allowed);

        for i in 0..(MAX_BUCKETS_PER_NAMESPACE + 1_000) {
            l.check(&format!("rl:tok{i}"), 60, 60);
        }

        let r = l.check("share:1.2.3.4", 1, 60);
        assert!(
            !r.allowed,
            "share:<ip> bucket must not be reset by an rl: flood"
        );
    }

    /// AC-S7's "latency stays stable" clause: batching the trim (see
    /// `trim_namespace`'s doc comment) means the O(n) scan only runs once
    /// per `10%` of the cap's worth of new keys, not on every single call
    /// once the namespace is full. A regression back to "evict one entry,
    /// scan on every call forever after" turns this into a multi-minute
    /// test (the bug report measured 225s for a comparable spray against an
    /// already-full map); this generous bound only exists to catch that
    /// class of regression, not to assert a tight performance number.
    #[test]
    fn eviction_stays_cheap_well_past_the_namespace_cap() {
        let l = Limiter::new();
        let started = Instant::now();
        for i in 0..(MAX_BUCKETS_PER_NAMESPACE * 2) {
            l.check(&format!("share:{i}"), 120, 60);
        }
        let elapsed = started.elapsed();
        assert!(
            elapsed.as_secs() < 5,
            "200k checks past the cap took {elapsed:?}; trimming must stay amortized O(1), not O(n) per call"
        );
    }

    #[test]
    fn namespace_of_splits_on_first_colon() {
        assert_eq!(namespace_of("rl:tok"), "rl");
        assert_eq!(namespace_of("rl:tok:7"), "rl");
        assert_eq!(namespace_of("share:1.2.3.4"), "share");
        assert_eq!(namespace_of("share:__global__"), "share");
        assert_eq!(namespace_of("no_colon"), "no_colon");
    }
}
