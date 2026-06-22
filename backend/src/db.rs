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

/// A trace record to persist off the response path (AC-46/47/57). Bodies are
/// already truncated to `MAX_BODY_BYTES` and headers already redacted by the
/// engine before this struct is built.
#[derive(Debug, Clone)]
pub struct TraceRecord {
    pub token: String,
    pub method: String,
    pub path: String,
    pub status_code: i64,
    pub served_by: String,
    pub matched_rule_id: Option<i64>,
    pub duration_ms: i64,
    pub overhead_ms: i64,
    pub request_headers: String, // JSON
    pub query_params: String,    // JSON
    pub request_body: Option<String>,
    pub response_headers: String, // JSON
    pub response_body: Option<String>,
    pub trace_json: String,    // JSON array of {step, detail}
    pub state_snapshot: String, // JSON
}

/// Insert a trace and prune to the newest `trace_cap` rows for that token
/// (write-time prune so the per-endpoint cap never drifts — AC-48). Returns the
/// new row id. Runs off the response path (spawned by the engine, never awaited).
pub async fn insert_trace(pool: &SqlitePool, rec: &TraceRecord, trace_cap: i64) -> Result<i64, sqlx::Error> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO request_logs
            (token, method, path, status_code, served_by, matched_rule_id,
             duration_ms, overhead_ms, request_headers, query_params, request_body,
             response_headers, response_body, trace_json, state_snapshot)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?) RETURNING id",
    )
    .bind(&rec.token)
    .bind(&rec.method)
    .bind(&rec.path)
    .bind(rec.status_code)
    .bind(&rec.served_by)
    .bind(rec.matched_rule_id)
    .bind(rec.duration_ms)
    .bind(rec.overhead_ms)
    .bind(&rec.request_headers)
    .bind(&rec.query_params)
    .bind(&rec.request_body)
    .bind(&rec.response_headers)
    .bind(&rec.response_body)
    .bind(&rec.trace_json)
    .bind(&rec.state_snapshot)
    .fetch_one(pool)
    .await?;

    // Write-time prune: keep only the newest `trace_cap` rows for this token.
    sqlx::query(
        "DELETE FROM request_logs
         WHERE token = ?
           AND id NOT IN (
               SELECT id FROM request_logs WHERE token = ? ORDER BY id DESC LIMIT ?
           )",
    )
    .bind(&rec.token)
    .bind(&rec.token)
    .bind(trace_cap)
    .execute(pool)
    .await?;

    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrate_then_round_trip_owner_endpoint() {
        let pool = pool(":memory:").await.unwrap();
        migrate(&pool).await.unwrap();

        // owners -> endpoints FK round trip, parameterized SQL (AC-S17).
        sqlx::query(
            "INSERT INTO owners (owner_id, email, secret_hash) VALUES (?, ?, ?)",
        )
        .bind("ownerabc12345678")
        .bind("a@b.com")
        .bind("hash")
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO endpoints (token, owner_id, name) VALUES (?, ?, ?)",
        )
        .bind("tok1234567")
        .bind("ownerabc12345678")
        .bind("demo")
        .execute(&pool)
        .await
        .unwrap();

        // Defaults from the §5.6 DDL apply.
        let (default_mode, latency, chaos_mode, cors, gone): (String, i64, String, i64, Option<String>) =
            sqlx::query_as(
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
