//! The `mock_rules` table: the typed rule row plus the rule queries. Rules
//! always come back in the frozen `priority, id` order.

use sqlx::SqlitePool;

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
