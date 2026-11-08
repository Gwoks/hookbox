//! Migration 0004 (drop `owners.email`, unique `secret_hash`) over a
//! database that already holds legacy email-era rows in every table that
//! hangs off `owners`.

use std::borrow::Cow;

use sqlx::migrate::Migrator;
use sqlx::SqlitePool;

use super::pool;
use crate::ids::hash_secret;

/// Apply only migrations `< version` — the schema as an older release left it.
async fn migrate_before(pool: &SqlitePool, version: i64) {
    let full = sqlx::migrate!("./migrations");
    let partial = Migrator {
        migrations: Cow::Owned(
            full.iter()
                .filter(|m| m.version < version)
                .cloned()
                .collect(),
        ),
        ..full
    };
    partial.run(pool).await.expect("apply pre-0004 migrations");
}

/// One legacy owner (WITH email) owning an endpoint, a rule, a request log
/// and a share link.
async fn seed_legacy(pool: &SqlitePool) {
    let statements = [
        "INSERT INTO owners (owner_id, email, secret_hash, created_at, last_seen)
         VALUES ('legacyowner00001', 'a@b.com', 'HASH1', '2026-01-01 00:00:00', '2026-02-01 00:00:00')",
        "INSERT INTO endpoints (token, owner_id, name) VALUES ('tokLegacy1', 'legacyowner00001', 'old')",
        "INSERT INTO mock_rules (token, name) VALUES ('tokLegacy1', 'r')",
        "INSERT INTO request_logs (token, method, path, status_code, served_by)
         VALUES ('tokLegacy1', 'GET', '/x', 200, 'rule')",
        "INSERT INTO share_links (code_hash, token) VALUES ('CODEHASH', 'tokLegacy1')",
    ];
    for sql in statements {
        sqlx::query(sql).execute(pool).await.expect(sql);
    }
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn drops_email_and_keeps_legacy_rows_and_foreign_keys() {
    let pool = pool(":memory:").await.unwrap();
    migrate_before(&pool, 4).await;
    seed_legacy(&pool).await;

    super::migrate(&pool).await.expect("apply 0004");

    let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('owners')")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(
        columns,
        vec!["owner_id", "secret_hash", "created_at", "last_seen"]
    );

    let (id, hash, created, seen): (String, String, String, Option<String>) =
        sqlx::query_as("SELECT owner_id, secret_hash, created_at, last_seen FROM owners")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(id, "legacyowner00001");
    assert_eq!(hash, "HASH1");
    assert_eq!(created, "2026-01-01 00:00:00");
    assert_eq!(seen.as_deref(), Some("2026-02-01 00:00:00"));
    for table in ["endpoints", "mock_rules", "request_logs", "share_links"] {
        assert_eq!(count(&pool, table).await, 1, "{table} row survives");
    }

    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(violations.len(), 0, "foreign_key_check reports orphans");
    let fk_on: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(fk_on, 1);

    // endpoints still references `owners` (not a renamed table).
    let ep_sql: String =
        sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE name = 'endpoints'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(ep_sql.contains("REFERENCES owners(owner_id)"), "{ep_sql}");
}

#[tokio::test]
async fn secret_hash_is_unique_and_endpoint_owner_fk_still_enforced() {
    let pool = pool(":memory:").await.unwrap();
    migrate_before(&pool, 4).await;
    seed_legacy(&pool).await;
    super::migrate(&pool).await.expect("apply 0004");

    let dup = sqlx::query("INSERT INTO owners (owner_id, secret_hash) VALUES ('other', 'HASH1')")
        .execute(&pool)
        .await;
    assert!(dup.is_err(), "duplicate secret_hash must be rejected");

    let orphan =
        sqlx::query("INSERT INTO endpoints (token, owner_id) VALUES ('tokOrphan1', 'nobody')")
            .execute(&pool)
            .await;
    assert!(
        orphan.is_err(),
        "endpoint with unknown owner must be rejected"
    );

    // A legacy secret keeps resolving through the new repo path.
    sqlx::query("UPDATE owners SET secret_hash = ? WHERE owner_id = 'legacyowner00001'")
        .bind(hash_secret("legacy-secret"))
        .execute(&pool)
        .await
        .unwrap();
    let found = crate::repo::owners::find_by_secret_hash(&pool, &hash_secret("legacy-secret"))
        .await
        .unwrap();
    assert_eq!(found.as_deref(), Some("legacyowner00001"));
}
