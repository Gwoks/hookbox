//! Per-endpoint state store over the `endpoint_state` table (replaces Redis
//! `state:<token>`). §5.5 / AC-20..23, AC-S17.
//!
//! Read = expiry-filtered `SELECT`; lazy — the engine reads only when some rule
//! gates on state. Write = upsert with `expires_at = now + STATE_TTL_SECONDS`.
//! Key charset `^[A-Za-z0-9_-]{1,64}$` is enforced before any write; an unsafe
//! key is skipped (never persisted). A SQLite fault surfaces as `sqlx::Error`
//! (the caller maps to 503); the state-gated match path fails closed by reading
//! an empty map.

use std::collections::BTreeMap;

use sqlx::{Row, SqlitePool};

use crate::helpers::is_safe_key;

/// Read the live (non-expired) state map for an endpoint.
pub async fn read_state(pool: &SqlitePool, token: &str) -> Result<BTreeMap<String, String>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT key, value FROM endpoint_state WHERE token = ? AND expires_at > datetime('now')",
    )
    .bind(token)
    .fetch_all(pool)
    .await?;
    let mut map = BTreeMap::new();
    for r in &rows {
        let k: String = r.get("key");
        let v: String = r.get("value");
        map.insert(k, v);
    }
    Ok(map)
}

/// Upsert one state key with a fresh TTL. Unsafe keys are silently skipped
/// (returns `Ok(false)`); a successful write returns `Ok(true)`.
pub async fn write_state(
    pool: &SqlitePool,
    token: &str,
    key: &str,
    value: &str,
    ttl_seconds: i64,
) -> Result<bool, sqlx::Error> {
    if !is_safe_key(key) {
        return Ok(false);
    }
    // SQLite modifier needs an explicit sign; `format!("+{n}")` breaks for n<0.
    let expires = format!("{ttl_seconds:+} seconds");
    sqlx::query(
        "INSERT INTO endpoint_state (token, key, value, expires_at)
         VALUES (?, ?, ?, datetime('now', ?))
         ON CONFLICT(token, key) DO UPDATE SET
             value = excluded.value,
             expires_at = excluded.expires_at",
    )
    .bind(token)
    .bind(key)
    .bind(value)
    .bind(&expires)
    .execute(pool)
    .await?;
    Ok(true)
}

/// Clear all state for an endpoint (management #16).
pub async fn clear_state(pool: &SqlitePool, token: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM endpoint_state WHERE token = ?")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    async fn pool_with_endpoint() -> SqlitePool {
        let pool = db::pool(":memory:").await.unwrap();
        db::migrate(&pool).await.unwrap();
        sqlx::query("INSERT INTO owners (owner_id, email, secret_hash) VALUES ('o','e','h')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO endpoints (token, owner_id) VALUES ('tok','o')")
            .execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn write_read_roundtrip_and_unsafe_skipped() {
        let pool = pool_with_endpoint().await;
        assert!(write_state(&pool, "tok", "logged_in", "1", 86400).await.unwrap());
        // unsafe key skipped, not persisted.
        assert!(!write_state(&pool, "tok", "bad key", "x", 86400).await.unwrap());
        let m = read_state(&pool, "tok").await.unwrap();
        assert_eq!(m.get("logged_in").map(String::as_str), Some("1"));
        assert!(!m.contains_key("bad key"));
        // upsert overwrites.
        write_state(&pool, "tok", "logged_in", "2", 86400).await.unwrap();
        assert_eq!(read_state(&pool, "tok").await.unwrap()["logged_in"], "2");
        clear_state(&pool, "tok").await.unwrap();
        assert!(read_state(&pool, "tok").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn expired_rows_filtered_out() {
        let pool = pool_with_endpoint().await;
        // negative TTL -> already expired.
        write_state(&pool, "tok", "stale", "x", -10).await.unwrap();
        assert!(read_state(&pool, "tok").await.unwrap().is_empty());
    }
}
