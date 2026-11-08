//! The `endpoints` table: the typed row every endpoint read decodes into,
//! plus the endpoint queries. A row that does not match the column types
//! surfaces as `sqlx::Error` (→ 503 at the route), never a panic.

use sqlx::SqlitePool;

use crate::ids::gen_token;

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

/// Whether the table holds any endpoint at all (first-run detection).
pub(crate) async fn any_exist(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let found: Option<i64> = sqlx::query_scalar("SELECT 1 FROM endpoints LIMIT 1")
        .fetch_optional(pool)
        .await?;
    Ok(found.is_some())
}

/// The columns a new endpoint row is created with (everything else takes its
/// schema default).
pub(crate) struct NewEndpoint<'a> {
    pub(crate) token: &'a str,
    pub(crate) owner_id: &'a str,
    pub(crate) name: Option<&'a str>,
    pub(crate) auto_crud: bool,
}

pub(crate) async fn insert(pool: &SqlitePool, new: &NewEndpoint<'_>) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO endpoints (token, owner_id, name, auto_crud) VALUES (?, ?, ?, ?)")
        .bind(new.token)
        .bind(new.owner_id)
        .bind(new.name)
        .bind(new.auto_crud)
        .execute(pool)
        .await?;
    Ok(())
}

/// How many times a colliding fresh token is re-drawn before inserting.
const TOKEN_ATTEMPTS: usize = 5;

/// Insert a fresh endpoint for an owner under a newly generated token,
/// re-drawing on the (astronomically rare) collision. Returns the token.
pub(crate) async fn create_with_fresh_token(
    pool: &SqlitePool,
    owner_id: &str,
    name: Option<&str>,
    token_len: usize,
) -> Result<String, sqlx::Error> {
    let mut token = gen_token(token_len);
    for _ in 0..TOKEN_ATTEMPTS {
        if owner_of(pool, &token).await?.is_none() {
            break;
        }
        token = gen_token(token_len);
    }
    let new = NewEndpoint {
        token: &token,
        owner_id,
        name,
        auto_crud: false,
    };
    insert(pool, &new).await?;
    Ok(token)
}

/// OQ-1: tombstone rather than hard-delete so the mock plane returns 410.
/// Clears the live config (rules / state / Auto-CRUD), revokes every share
/// link, then sets `gone_at`.
pub(crate) async fn tombstone(pool: &SqlitePool, token: &str) -> Result<(), sqlx::Error> {
    for sql in [
        "DELETE FROM mock_rules WHERE token = ?",
        "DELETE FROM endpoint_state WHERE token = ?",
        "DELETE FROM crud_collections WHERE token = ?",
        // F4/AC-S9: tombstone-revokes-all-shares, BEFORE gone_at is set, so
        // the public resolver's belt-and-braces `gone_at IS NULL` check and
        // this revoke agree even if one of the two statements were to fail.
        "UPDATE share_links SET revoked_at = datetime('now') WHERE token = ? AND revoked_at IS NULL",
        "UPDATE endpoints SET gone_at = datetime('now') WHERE token = ?",
    ] {
        sqlx::query(sql).bind(token).execute(pool).await?;
    }
    Ok(())
}
