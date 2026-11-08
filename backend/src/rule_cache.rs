//! Compiled-endpoint cache — PORT of `app/rule_cache.py` (AC-11/15/38, AC-S18).
//!
//! On the P1 fast path the endpoint config + compiled rules come from this
//! in-process cache: it cold-loads an endpoint once from SQLite, compiles its
//! rules (sorted by priority,id), and serves subsequent requests with NO
//! per-request DB read on a match. Any management write calls `invalidate(token)`
//! so the next P1 request cold-loads fresh config. A tombstoned endpoint
//! (`gone_at` non-null) is cached as `Gone` so 410 is served without a DB read.

use std::sync::Arc;

use dashmap::DashMap;
use sqlx::SqlitePool;

use crate::interceptor::matcher::CompiledRule;
use crate::repo::endpoints::{self, EndpointRow};
use crate::repo::rules::{self, RuleRow};

/// A compiled endpoint ready for the fast path.
#[derive(Clone)]
pub struct CompiledEndpoint {
    pub token: String,
    pub auto_crud: bool,
    pub target_url: Option<String>,
    pub default_mode: String, // "mock_404" | "echo"
    pub latency_ms: i64,
    pub rate_limit_per_min: i64,
    pub chaos_pct: i64,
    pub chaos_mode: String, // "error" | "dropout"
    pub cors_enabled: bool,
    pub rules: Arc<Vec<CompiledRule>>,
    pub any_rule_gates_on_state: bool,
}

/// What the cache resolves a token to.
#[derive(Clone)]
pub enum Resolved {
    /// Live endpoint with compiled rules.
    Live(Arc<CompiledEndpoint>),
    /// Tombstoned (deleted/expired) — serve 410 without a DB read.
    Gone,
}

pub struct RuleCache {
    entries: DashMap<String, Resolved>,
}

impl Default for RuleCache {
    fn default() -> Self {
        RuleCache {
            entries: DashMap::new(),
        }
    }
}

impl RuleCache {
    pub fn new() -> Self {
        RuleCache::default()
    }

    /// Drop the cached entry for a token after any management write.
    pub fn invalidate(&self, token: &str) {
        self.entries.remove(token);
    }

    /// Resolve a token: serve from cache, else cold-load from SQLite. Returns
    /// `None` for a genuinely unknown endpoint (no row at all → 404).
    pub async fn get(
        &self,
        pool: &SqlitePool,
        token: &str,
    ) -> Result<Option<Resolved>, sqlx::Error> {
        if let Some(hit) = self.entries.get(token) {
            return Ok(Some(hit.clone()));
        }
        let Some(row) = endpoints::find(pool, token).await? else {
            return Ok(None); // unknown -> 404 (not cached)
        };
        if row.is_gone() {
            self.entries.insert(token.to_string(), Resolved::Gone);
            return Ok(Some(Resolved::Gone));
        }
        let rule_rows = rules::list(pool, token).await?;
        let resolved = Resolved::Live(Arc::new(CompiledEndpoint::from_rows(&row, &rule_rows)));
        self.entries.insert(token.to_string(), resolved.clone());
        Ok(Some(resolved))
    }
}

impl CompiledEndpoint {
    /// Compile a live endpoint row plus its rules (already in `priority, id`
    /// order) for the fast path.
    pub(crate) fn from_rows(row: &EndpointRow, rule_rows: &[RuleRow]) -> Self {
        let rules: Vec<CompiledRule> = rule_rows.iter().map(CompiledRule::from_row).collect();
        CompiledEndpoint {
            token: row.token.clone(),
            auto_crud: row.auto_crud,
            target_url: row.target_url.clone(),
            default_mode: row.default_mode.clone(),
            latency_ms: row.latency_ms,
            rate_limit_per_min: row.rate_limit_per_min,
            chaos_pct: row.chaos_pct,
            chaos_mode: row.chaos_mode.clone(),
            cors_enabled: row.cors_enabled,
            any_rule_gates_on_state: rules.iter().any(CompiledRule::gates_on_state),
            rules: Arc::new(rules),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{memory_pool, seed_owner};

    #[tokio::test]
    async fn cold_load_compile_and_invalidate() {
        let p = memory_pool().await;
        let owner = seed_owner(&p).await;
        sqlx::query("INSERT INTO endpoints (token, owner_id, auto_crud) VALUES ('tok',?,1)")
            .bind(&owner.owner_id)
            .execute(&p)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO mock_rules (token, priority, enabled, match_json, response_json, state_writes_json)
             VALUES ('tok', 50, 1, '{\"method\":\"GET\",\"path\":\"/*\"}', '{\"status_code\":200}', '[]')",
        ).execute(&p).await.unwrap();
        let cache = RuleCache::new();
        let r = cache.get(&p, "tok").await.unwrap().unwrap();
        match r {
            Resolved::Live(ep) => {
                assert!(ep.auto_crud);
                assert_eq!(ep.rules.len(), 1);
                assert_eq!(ep.rules[0].priority, 50);
            }
            _ => panic!("expected live"),
        }
        // unknown -> None
        assert!(cache.get(&p, "nope").await.unwrap().is_none());
        // tombstone -> Gone
        sqlx::query("UPDATE endpoints SET gone_at = datetime('now') WHERE token='tok'")
            .execute(&p)
            .await
            .unwrap();
        cache.invalidate("tok");
        assert!(matches!(
            cache.get(&p, "tok").await.unwrap().unwrap(),
            Resolved::Gone
        ));
    }
}
