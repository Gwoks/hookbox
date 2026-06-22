//! First-run demo-data seeding — PORT of the intent of the Python seed (AC-53/54,
//! AC-J12). Plants a demo owner + a primary endpoint with >=1 rule + sample
//! traces so a fresh dashboard lands populated. Idempotent: a no-op when any
//! endpoint already exists.

use serde_json::json;
use sqlx::SqlitePool;

use crate::ids::{gen_owner_secret, gen_token, hash_email, hash_secret};

/// Seed demo data if the DB is empty. Returns `Some((email, secret, token))` when
/// it seeded (first run), `None` when data already existed.
pub async fn seed_if_empty(pool: &SqlitePool, id_len: usize) -> Result<Option<(String, String, String)>, sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM endpoints")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(None);
    }

    let email = "demo@hookbox.local".to_string();
    let owner_id = hash_email(&email);
    let secret = gen_owner_secret(32);
    sqlx::query(
        "INSERT INTO owners (owner_id, email, secret_hash, last_seen)
         VALUES (?, ?, ?, datetime('now'))
         ON CONFLICT(owner_id) DO UPDATE SET secret_hash = excluded.secret_hash",
    )
    .bind(&owner_id)
    .bind(&email)
    .bind(hash_secret(&secret))
    .execute(pool)
    .await?;

    let token = gen_token(id_len);
    sqlx::query("INSERT INTO endpoints (token, owner_id, name, auto_crud) VALUES (?, ?, 'Demo API', 1)")
        .bind(&token)
        .bind(&owner_id)
        .execute(pool)
        .await?;

    // A demo rule: GET /hello -> 200 {"hello":"world"} with a template tag.
    let match_json = json!({"method":"GET","path":"/hello"}).to_string();
    let response_json = json!({
        "status_code": 200,
        "content_type": "application/json",
        "body_template": "{\"hello\":\"world\",\"at\":\"{{now 'iso'}}\"}",
        "headers": {}
    })
    .to_string();
    sqlx::query(
        "INSERT INTO mock_rules (token, name, priority, enabled, match_json, response_json, state_writes_json)
         VALUES (?, 'Hello rule', 100, 1, ?, ?, '[]')",
    )
    .bind(&token)
    .bind(&match_json)
    .bind(&response_json)
    .execute(pool)
    .await?;

    // A couple of sample traces so the first-run feed is populated.
    for (i, (path, sb)) in [("/hello", "rule"), ("/widgets", "crud")].iter().enumerate() {
        sqlx::query(
            "INSERT INTO request_logs (token, method, path, status_code, served_by, duration_ms, overhead_ms,
                 request_headers, query_params, response_headers, trace_json, state_snapshot)
             VALUES (?, 'GET', ?, 200, ?, ?, ?, '{}', '{}', '{}', '[]', '{}')",
        )
        .bind(&token)
        .bind(path)
        .bind(sb)
        .bind((i as i64 + 1) * 2)
        .bind(i as i64 + 1)
        .execute(pool)
        .await?;
    }

    Ok(Some((email, secret, token)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[tokio::test]
    async fn seeds_once_then_idempotent() {
        let pool = db::pool(":memory:").await.unwrap();
        db::migrate(&pool).await.unwrap();
        let first = seed_if_empty(&pool, 10).await.unwrap();
        assert!(first.is_some());
        let (_e, _s, token) = first.unwrap();
        let rules: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mock_rules WHERE token = ?")
            .bind(&token).fetch_one(&pool).await.unwrap();
        assert_eq!(rules, 1);
        let traces: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM request_logs WHERE token = ?")
            .bind(&token).fetch_one(&pool).await.unwrap();
        assert_eq!(traces, 2);
        // second run is a no-op.
        assert!(seed_if_empty(&pool, 10).await.unwrap().is_none());
    }
}
