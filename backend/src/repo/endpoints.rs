//! The `endpoints` table: the typed row every endpoint read decodes into,
//! plus the endpoint queries. A row that does not match the column types
//! surfaces as `sqlx::Error` (→ 503 at the route), never a panic.

use sqlx::SqlitePool;

/// One `endpoints` row. Boolean columns decode from SQLite INTEGER; the
/// enum-like `default_mode` / `chaos_mode` stay `String` (parsed where used).
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct EndpointRow {
    pub(crate) token: String,
    pub(crate) owner_id: String,
    pub(crate) name: Option<String>,
    pub(crate) auto_crud: bool,
    pub(crate) target_url: Option<String>,
    pub(crate) default_mode: String,
    pub(crate) latency_ms: i64,
    pub(crate) rate_limit_per_min: i64,
    pub(crate) chaos_pct: i64,
    pub(crate) chaos_mode: String,
    pub(crate) cors_enabled: bool,
    pub(crate) gone_at: Option<String>,
    pub(crate) created_at: String,
    pub(crate) last_hit: Option<String>,
    pub(crate) request_count: i64,
}

impl EndpointRow {
    /// Tombstoned (deleted) — the mock plane answers 410 for it.
    pub(crate) fn is_gone(&self) -> bool {
        self.gone_at.is_some()
    }
}

/// The endpoint `token`, tombstoned or not.
pub(crate) async fn find(
    pool: &SqlitePool,
    token: &str,
) -> Result<Option<EndpointRow>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM endpoints WHERE token = ?")
        .bind(token)
        .fetch_optional(pool)
        .await
}

/// The owner of `token` (tombstoned endpoints included), if it exists.
pub(crate) async fn owner_of(
    pool: &SqlitePool,
    token: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT owner_id FROM endpoints WHERE token = ?")
        .bind(token)
        .fetch_optional(pool)
        .await
}

/// Every endpoint the owner holds (tombstones included), oldest first.
pub(crate) async fn list_by_owner(
    pool: &SqlitePool,
    owner_id: &str,
) -> Result<Vec<EndpointRow>, sqlx::Error> {
    sqlx::query_as("SELECT * FROM endpoints WHERE owner_id = ? ORDER BY created_at, token")
        .bind(owner_id)
        .fetch_all(pool)
        .await
}
