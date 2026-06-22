//! Shared Axum application state (§ Component design). Cloneable handle passed
//! to every handler. Mirrors `../shortener-link/backend/src/state.rs`.
//!
//! `AppState { pool, cfg, rule_cache, limiter, feed_hub, tunnels }` is the
//! frozen shape from architecture.md. The scaffold (hookbox-sks.11) defines the
//! struct and minimal placeholder components; the rule cache (sks.15), limiter
//! (sks.20), feed hub (sks.21) and tunnel registry (sks.23) tasks flesh out the
//! behavior of their respective modules behind these same fields.

use std::sync::Arc;

use sqlx::SqlitePool;

use crate::config::Config;
use crate::feed::FeedHub;
use crate::limiter::Limiter;
use crate::rule_cache::RuleCache;
use crate::tunnels::TunnelRegistry;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cfg: Arc<Config>,
    /// In-process compiled-endpoint cache (no per-request DB read on a P1 match).
    pub rule_cache: Arc<RuleCache>,
    /// In-memory token-bucket limiter (DashMap, fail-open).
    pub limiter: Arc<Limiter>,
    /// Live-feed broadcast hub (one `broadcast::Sender` per token).
    pub feed_hub: Arc<FeedHub>,
    /// In-process tunnel registry (`token -> TunnelConnection`).
    pub tunnels: Arc<TunnelRegistry>,
}

impl AppState {
    /// Build state from an open pool + config, wiring fresh in-process components.
    pub fn new(pool: SqlitePool, cfg: Config) -> Self {
        AppState {
            pool,
            cfg: Arc::new(cfg),
            rule_cache: Arc::new(RuleCache::new()),
            limiter: Arc::new(Limiter::new()),
            feed_hub: Arc::new(FeedHub::new()),
            tunnels: Arc::new(TunnelRegistry::new()),
        }
    }
}
