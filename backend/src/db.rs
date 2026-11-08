//! SQLite pool + pragmas + migrations (§5.6: WAL, foreign_keys, busy_timeout).
//! Mirrors `../shortener-link/backend/src/db.rs`.

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::str::FromStr;
use std::time::Duration;

/// Open (creating if missing) a WAL-mode SQLite pool with foreign keys enforced.
pub async fn pool(sqlite_path: &str) -> anyhow::Result<SqlitePool> {
    let url = if sqlite_path == ":memory:" {
        "sqlite::memory:".to_string()
    } else {
        format!("sqlite://{sqlite_path}")
    };
    let opts = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await?;
    Ok(pool)
}

/// Run embedded migrations from ./migrations on startup (AC-53).
pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

#[cfg(test)]
mod migration_0004_tests;

#[cfg(test)]
mod tests {
    use crate::testing::{memory_pool, seed_endpoint, seed_owner};

    #[tokio::test]
    async fn migrate_then_round_trip_owner_endpoint() {
        // `memory_pool` is `pool(":memory:")` + `migrate`.
        let pool = memory_pool().await;

        // owners -> endpoints FK round trip, parameterized SQL (AC-S17).
        let owner = seed_owner(&pool).await;
        seed_endpoint(&pool, &owner.owner_id, "tok1234567").await;

        // Defaults from the §5.6 DDL apply.
        let (default_mode, latency, chaos_mode, cors, gone): (
            String,
            i64,
            String,
            i64,
            Option<String>,
        ) = sqlx::query_as(
            "SELECT default_mode, latency_ms, chaos_mode, cors_enabled, gone_at
                 FROM endpoints WHERE token = ?",
        )
        .bind("tok1234567")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(default_mode, "mock_404");
        assert_eq!(latency, 0);
        assert_eq!(chaos_mode, "error"); // OQ-2 default
        assert_eq!(cors, 1);
        assert!(gone.is_none()); // OQ-1 tombstone: live by default

        // The three Redis-replacement tables exist (state + crud).
        sqlx::query(
            "INSERT INTO endpoint_state (token, key, value, expires_at)
             VALUES (?, ?, ?, datetime('now','+1 day'))",
        )
        .bind("tok1234567")
        .bind("k")
        .bind("v")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO crud_collections (token, name, expires_at)
             VALUES (?, ?, datetime('now','+1 day'))",
        )
        .bind("tok1234567")
        .bind("widgets")
        .execute(&pool)
        .await
        .unwrap();

        // ON DELETE CASCADE: deleting the endpoint clears state/crud/rules/logs.
        sqlx::query("DELETE FROM endpoints WHERE token = ?")
            .bind("tok1234567")
            .execute(&pool)
            .await
            .unwrap();
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM endpoint_state")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 0);
    }
}
