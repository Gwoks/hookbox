//! The retention sweep's SQL (AC-46..48, AC-58/59/61): each function is one
//! statement and returns how many rows it removed.

use sqlx::SqlitePool;

async fn delete_with(
    pool: &SqlitePool,
    sql: &'static str,
    arg: Option<String>,
) -> Result<u64, sqlx::Error> {
    let query = sqlx::query(sql);
    let query = match arg {
        Some(a) => query.bind(a),
        None => query,
    };
    Ok(query.execute(pool).await?.rows_affected())
}

/// Traces older than `hours` (TRACE_TTL_HOURS).
pub(crate) async fn prune_traces_older_than(
    pool: &SqlitePool,
    hours: i64,
) -> Result<u64, sqlx::Error> {
    let sql = "DELETE FROM request_logs WHERE created_at < datetime('now', ?)";
    delete_with(pool, sql, Some(format!("-{hours} hours"))).await
}

/// Everything beyond the newest `cap` traces of each token (TRACE_CAP).
pub(crate) async fn cap_traces_per_token(pool: &SqlitePool, cap: i64) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM request_logs
         WHERE id NOT IN (
             SELECT id FROM (
                 SELECT id, ROW_NUMBER() OVER (PARTITION BY token ORDER BY id DESC) AS rn
                 FROM request_logs
             ) WHERE rn <= ?
         )",
    )
    .bind(cap)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Expired `endpoint_state` rows.
pub(crate) async fn reap_expired_state(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let sql = "DELETE FROM endpoint_state WHERE expires_at <= datetime('now')";
    delete_with(pool, sql, None).await
}

/// Expired Auto-CRUD collections.
pub(crate) async fn reap_expired_crud(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let sql = "DELETE FROM crud_collections WHERE expires_at <= datetime('now')";
    delete_with(pool, sql, None).await
}

/// Hard-delete tombstones older than `hours` (GONE_TTL_HOURS); the token
/// then degrades from 410 to 404.
pub(crate) async fn reap_tombstones_older_than(
    pool: &SqlitePool,
    hours: i64,
) -> Result<u64, sqlx::Error> {
    let sql = "DELETE FROM endpoints WHERE gone_at IS NOT NULL AND gone_at < datetime('now', ?)";
    delete_with(pool, sql, Some(format!("-{hours} hours"))).await
}
