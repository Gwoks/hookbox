//! The ONLY SQL that touches the `owners` table (plus the owner-scoped
//! endpoint reads the session summary needs). An owner is a non-secret
//! random `owner_id` and `secret_hash = sha256(owner key)`; the raw key is
//! never stored or compared directly — callers hash first (`ids::hash_secret`).

use sqlx::sqlite::SqliteRow;
use sqlx::SqlitePool;

/// Insert a brand-new owner, stamping `last_seen` now.
pub(crate) async fn insert(
    pool: &SqlitePool,
    owner_id: &str,
    key_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO owners (owner_id, secret_hash, last_seen) VALUES (?, ?, datetime('now'))",
    )
    .bind(owner_id)
    .bind(key_hash)
    .execute(pool)
    .await?;
    Ok(())
}

/// The owner holding the key whose sha256 is `key_hash`, if any (one probe
/// on the unique `idx_owners_secret_hash` index).
pub(crate) async fn find_by_secret_hash(
    pool: &SqlitePool,
    key_hash: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT owner_id FROM owners WHERE secret_hash = ?")
        .bind(key_hash)
        .fetch_optional(pool)
        .await
}

/// Replace an owner's key hash (explicit rotate). The previous key stops
/// resolving immediately.
pub(crate) async fn set_key_hash(
    pool: &SqlitePool,
    owner_id: &str,
    key_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE owners SET secret_hash = ? WHERE owner_id = ?")
        .bind(key_hash)
        .bind(owner_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Bump `last_seen`, at most once an hour (a no-op write otherwise), so a
/// future retention sweep can find abandoned anonymous owners without every
/// page load costing a row write.
pub(crate) async fn touch_last_seen(pool: &SqlitePool, owner_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE owners SET last_seen = datetime('now')
          WHERE owner_id = ?
            AND (last_seen IS NULL OR last_seen < datetime('now', '-1 hour'))",
    )
    .bind(owner_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// The owner's live (non-tombstoned) endpoints, oldest first — the first is
/// the session's `primary`.
pub(crate) async fn live_endpoints(
    pool: &SqlitePool,
    owner_id: &str,
) -> Result<Vec<SqliteRow>, sqlx::Error> {
    sqlx::query(
        "SELECT * FROM endpoints
          WHERE owner_id = ? AND gone_at IS NULL
          ORDER BY created_at, token",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await
}

/// Every endpoint token the owner holds, tombstoned ones included — the set
/// whose tunnels a key rotation must revoke.
pub(crate) async fn endpoint_tokens(
    pool: &SqlitePool,
    owner_id: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT token FROM endpoints WHERE owner_id = ?")
        .bind(owner_id)
        .fetch_all(pool)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{gen_owner_id, hash_secret};
    use crate::testing::{memory_pool, seed_endpoint};

    #[tokio::test]
    async fn insert_find_and_rotate() {
        let pool = memory_pool().await;
        let id = gen_owner_id();
        insert(&pool, &id, &hash_secret("k1")).await.unwrap();
        assert_eq!(
            find_by_secret_hash(&pool, &hash_secret("k1"))
                .await
                .unwrap(),
            Some(id.clone())
        );
        set_key_hash(&pool, &id, &hash_secret("k2")).await.unwrap();
        assert!(find_by_secret_hash(&pool, &hash_secret("k1"))
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            find_by_secret_hash(&pool, &hash_secret("k2"))
                .await
                .unwrap(),
            Some(id)
        );
    }

    #[tokio::test]
    async fn touch_last_seen_is_throttled_to_once_an_hour() {
        let pool = memory_pool().await;
        insert(&pool, "o1", &hash_secret("k")).await.unwrap();
        let stale = "2000-01-01 00:00:00";
        sqlx::query("UPDATE owners SET last_seen = ? WHERE owner_id = 'o1'")
            .bind(stale)
            .execute(&pool)
            .await
            .unwrap();
        touch_last_seen(&pool, "o1").await.unwrap();
        let seen: String = sqlx::query_scalar("SELECT last_seen FROM owners WHERE owner_id = 'o1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_ne!(seen, stale, "an hour-old last_seen is refreshed");

        // A fresh value (< 1h old) is left alone.
        let recent = sqlx::query_scalar::<_, String>("SELECT datetime('now', '-5 minutes')")
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE owners SET last_seen = ? WHERE owner_id = 'o1'")
            .bind(&recent)
            .execute(&pool)
            .await
            .unwrap();
        touch_last_seen(&pool, "o1").await.unwrap();
        let seen: String = sqlx::query_scalar("SELECT last_seen FROM owners WHERE owner_id = 'o1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(seen, recent);
    }

    #[tokio::test]
    async fn live_endpoints_exclude_tombstones_but_tokens_include_them() {
        let pool = memory_pool().await;
        insert(&pool, "o1", &hash_secret("k")).await.unwrap();
        seed_endpoint(&pool, "o1", "tokLive111").await;
        seed_endpoint(&pool, "o1", "tokGone222").await;
        sqlx::query("UPDATE endpoints SET gone_at = datetime('now') WHERE token = 'tokGone222'")
            .execute(&pool)
            .await
            .unwrap();
        let live = live_endpoints(&pool, "o1").await.unwrap();
        assert_eq!(live.len(), 1);
        let mut all = endpoint_tokens(&pool, "o1").await.unwrap();
        all.sort();
        assert_eq!(all, vec!["tokGone222", "tokLive111"]);
    }
}
