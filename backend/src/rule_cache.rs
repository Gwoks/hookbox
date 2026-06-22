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
use sqlx::{Row, SqlitePool};

use crate::interceptor::matcher::{compile_rule, CompiledRule};
use crate::models::{MatchCriteria, ResponseSpec, StateWrite, WebhookAction};

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
        let row = sqlx::query("SELECT * FROM endpoints WHERE token = ?")
            .bind(token)
            .fetch_optional(pool)
            .await?;
        let row = match row {
            Some(r) => r,
            None => return Ok(None), // unknown -> 404 (not cached)
        };
        let gone_at: Option<String> = row.get("gone_at");
        if gone_at.is_some() {
            self.entries.insert(token.to_string(), Resolved::Gone);
            return Ok(Some(Resolved::Gone));
        }

        let rule_rows =
            sqlx::query("SELECT * FROM mock_rules WHERE token = ? ORDER BY priority, id")
                .bind(token)
                .fetch_all(pool)
                .await?;
        let mut rules = Vec::with_capacity(rule_rows.len());
        let mut any_state = false;
        for r in &rule_rows {
            let match_json: String = r.get("match_json");
            let response_json: String = r.get("response_json");
            let writes_json: String = r.get("state_writes_json");
            let webhook_json: Option<String> = r.get("webhook_json");
            let mc: MatchCriteria = serde_json::from_str(&match_json).unwrap_or_default();
            let rsp: ResponseSpec = serde_json::from_str(&response_json).unwrap_or_default();
            let writes: Vec<StateWrite> = serde_json::from_str(&writes_json).unwrap_or_default();
            let webhook: Option<WebhookAction> =
                webhook_json.and_then(|w| serde_json::from_str(&w).ok());
            let compiled = compile_rule(
                r.get("id"),
                r.get("priority"),
                r.get::<i64, _>("enabled") != 0,
                &mc,
                rsp,
                writes,
                r.get("latency_ms"),
                r.get("rate_limit_per_min"),
                r.get("chaos_mode"),
                webhook,
            );
            if compiled.gates_on_state() {
                any_state = true;
            }
            rules.push(compiled);
        }

        let ep = Arc::new(CompiledEndpoint {
            token: token.to_string(),
            auto_crud: row.get::<i64, _>("auto_crud") != 0,
            target_url: row.get("target_url"),
            default_mode: row.get("default_mode"),
            latency_ms: row.get("latency_ms"),
            rate_limit_per_min: row.get("rate_limit_per_min"),
            chaos_pct: row.get("chaos_pct"),
            chaos_mode: row.get("chaos_mode"),
            cors_enabled: row.get::<i64, _>("cors_enabled") != 0,
            rules: Arc::new(rules),
            any_rule_gates_on_state: any_state,
        });
        let resolved = Resolved::Live(ep);
        self.entries.insert(token.to_string(), resolved.clone());
        Ok(Some(resolved))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    async fn pool() -> SqlitePool {
        let p = db::pool(":memory:").await.unwrap();
        db::migrate(&p).await.unwrap();
        sqlx::query("INSERT INTO owners (owner_id, email, secret_hash) VALUES ('o','e','h')")
            .execute(&p)
            .await
            .unwrap();
        p
    }

    #[tokio::test]
    async fn cold_load_compile_and_invalidate() {
        let p = pool().await;
        sqlx::query("INSERT INTO endpoints (token, owner_id, auto_crud) VALUES ('tok','o',1)")
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
