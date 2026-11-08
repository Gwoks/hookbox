//! The `mock_rules` table: the typed rule row plus the rule queries. Rules
//! always come back in the frozen `priority, id` order.

use sqlx::types::Json;
use sqlx::SqlitePool;

use super::UpdateBuilder;
use crate::models::{MockRuleCreate, RulePatch};

/// One `mock_rules` row. The `*_json` columns hold the serialized
/// `MatchCriteria` / `ResponseSpec` / `StateWrite[]` / `WebhookAction` and
/// are parsed leniently by the presenters (a corrupt column degrades to its
/// documented empty shape).
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct RuleRow {
    pub(crate) id: i64,
    pub(crate) token: String,
    pub(crate) name: Option<String>,
    pub(crate) priority: i64,
    pub(crate) enabled: bool,
    pub(crate) match_json: String,
    pub(crate) response_json: String,
    pub(crate) state_writes_json: String,
    pub(crate) latency_ms: Option<i64>,
    pub(crate) rate_limit_per_min: Option<i64>,
    pub(crate) chaos_mode: Option<String>,
    pub(crate) webhook_json: Option<String>,
    pub(crate) created_at: String,
}

/// Every rule on `token`, in `priority, id` order.
pub(crate) async fn list(pool: &SqlitePool, token: &str) -> Result<Vec<RuleRow>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM mock_rules WHERE token = ? ORDER BY priority, id")
        .bind(token)
        .fetch_all(pool)
        .await
}

/// Rule `id`, only if it belongs to `token`.
pub(crate) async fn find(
    pool: &SqlitePool,
    token: &str,
    id: i64,
) -> Result<Option<RuleRow>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM mock_rules WHERE id = ? AND token = ?")
        .bind(id)
        .bind(token)
        .fetch_optional(pool)
        .await
}

/// Insert an already-validated (and clamped) rule on `token`; returns the
/// stored row. The JSON columns are bound as `Json` — the exact bytes of
/// `serde_json::to_string`, with no fallible serialization at the call site.
pub(crate) async fn insert(
    pool: &SqlitePool,
    token: &str,
    rule: &MockRuleCreate,
) -> Result<RuleRow, sqlx::Error> {
    sqlx::query_as(
        "INSERT INTO mock_rules
            (token, name, priority, enabled, match_json, response_json,
             state_writes_json, latency_ms, rate_limit_per_min, chaos_mode, webhook_json)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(token)
    .bind(&rule.name)
    .bind(rule.priority)
    .bind(rule.enabled)
    .bind(Json(&rule.r#match))
    .bind(Json(&rule.response))
    .bind(Json(&rule.state_writes))
    .bind(rule.latency_ms)
    .bind(rule.rate_limit_per_min)
    .bind(&rule.chaos_mode)
    .bind(rule.webhook_action.as_ref().map(Json))
    .fetch_one(pool)
    .await
}

/// Apply a validated PATCH to rule `id` on `token`; the updated row, or
/// `None` when the patch set nothing (no statement is run).
pub(crate) async fn update(
    pool: &SqlitePool,
    token: &str,
    id: i64,
    patch: RulePatch,
) -> Result<Option<RuleRow>, sqlx::Error> {
    let mut b = UpdateBuilder::new("mock_rules");
    b.set_some("name", patch.name)
        .set_some("priority", patch.priority)
        .set_some("enabled", patch.enabled)
        .set_some("match_json", patch.r#match.map(Json))
        .set_some("response_json", patch.response.map(Json))
        .set_some("state_writes_json", patch.state_writes.map(Json))
        .set_some("latency_ms", patch.latency_ms)
        .set_some("rate_limit_per_min", patch.rate_limit_per_min)
        .set_some("chaos_mode", patch.chaos_mode)
        .set_some("webhook_json", patch.webhook_action.map(|w| w.map(Json)))
        .where_eq("id", id)
        .where_eq("token", token);
    b.fetch_returning(pool).await
}

/// Delete rule `id` on `token`; `false` when there was no such rule.
pub(crate) async fn delete(pool: &SqlitePool, token: &str, id: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM mock_rules WHERE id = ? AND token = ?")
        .bind(id)
        .bind(token)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
