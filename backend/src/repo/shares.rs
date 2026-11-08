//! `share_links` (F4): the typed share rows and every share-link query. Only
//! `code_hash` (sha256 of the code) is ever stored or looked up — the code
//! itself never reaches this module.

use sqlx::SqlitePool;

/// An owner-visible share link: no code, no hash (AC-25).
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct ShareRow {
    pub(crate) id: i64,
    pub(crate) label: Option<String>,
    pub(crate) created_at: String,
    pub(crate) last_used_at: Option<String>,
}

/// A live share joined to its live endpoint (§5.2's resolution statement).
#[derive(Debug, Clone, sqlx::FromRow)]
pub(crate) struct ResolvedShare {
    pub(crate) share_id: i64,
    pub(crate) token: String,
    pub(crate) endpoint_name: Option<String>,
    pub(crate) endpoint_created_at: String,
    pub(crate) request_count: i64,
}

/// `token`'s active (unrevoked) links, newest first.
pub(crate) async fn list_active(
    pool: &SqlitePool,
    token: &str,
) -> Result<Vec<ShareRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, label, created_at, last_used_at FROM share_links
          WHERE token = ? AND revoked_at IS NULL ORDER BY created_at DESC, id DESC",
    )
    .bind(token)
    .fetch_all(pool)
    .await
}

/// One round trip, joining liveness so a tombstone is indistinguishable from
/// an unknown/revoked code (AC-S9): `s.revoked_at IS NULL AND e.gone_at IS
/// NULL`, both read fresh on every request (never cached — `rule_cache` is
/// keyed by token for the mock plane only and gets no share entries, which is
/// what makes revocation take effect on the very next request, AC-37).
pub(crate) async fn resolve(
    pool: &SqlitePool,
    code_hash: &str,
) -> Result<Option<ResolvedShare>, sqlx::Error> {
    sqlx::query_as(
        "SELECT s.id AS share_id, s.token, e.name AS endpoint_name,
                e.created_at AS endpoint_created_at, e.request_count
           FROM share_links s
           JOIN endpoints  e ON e.token = s.token
          WHERE s.code_hash = ?
            AND s.revoked_at IS NULL
            AND e.gone_at   IS NULL",
    )
    .bind(code_hash)
    .fetch_optional(pool)
    .await
}
