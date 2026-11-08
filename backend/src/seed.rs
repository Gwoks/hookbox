//! First-run demo-data seeding — PORT of the intent of the Python seed (AC-53/54,
//! AC-J12). Plants an anonymous demo owner + a primary endpoint with >=1 rule +
//! sample traces so a fresh dashboard lands populated. Idempotent: a no-op when
//! any endpoint already exists.

use serde_json::json;
use sqlx::SqlitePool;

use crate::ids::{gen_owner_id, gen_owner_secret, gen_token, hash_secret};
use crate::repo::owners;

/// Entropy of the demo owner's key (the `OWNER_SECRET_BYTES` floor).
const DEMO_KEY_BYTES: usize = 32;

/// What a first-run seed planted. `key` is the demo owner's recovery key —
/// a live credential, so callers must only ever show it to the operator
/// (the `seed` bin prints it to stderr once; the server never logs it).
pub struct SeededOwner {
    pub owner_id: String,
    pub key: String,
    pub token: String,
}

/// Seed demo data if the DB is empty. Returns the planted owner on the
/// first run, `None` when data already existed.
pub async fn seed_if_empty(
    pool: &SqlitePool,
    id_len: usize,
) -> Result<Option<SeededOwner>, sqlx::Error> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM endpoints")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(None);
    }

    let owner_id = gen_owner_id();
    let key = gen_owner_secret(DEMO_KEY_BYTES);
    owners::insert(pool, &owner_id, &hash_secret(&key)).await?;

    let token = gen_token(id_len);
    sqlx::query(
        "INSERT INTO endpoints (token, owner_id, name, auto_crud) VALUES (?, ?, 'Demo API', 1)",
    )
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
    for (i, (path, sb)) in [("/hello", "rule"), ("/widgets", "crud")]
        .iter()
        .enumerate()
    {
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

    Ok(Some(SeededOwner {
        owner_id,
        key,
        token,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::memory_pool;

    #[tokio::test]
    async fn seeds_once_then_idempotent() {
        let pool = memory_pool().await;
        let SeededOwner {
            owner_id,
            key,
            token,
        } = seed_if_empty(&pool, 10)
            .await
            .unwrap()
            .expect("first run seeds");
        // The printed key really is the demo owner's credential.
        let resolved = owners::find_by_secret_hash(&pool, &hash_secret(&key))
            .await
            .unwrap();
        assert_eq!(resolved, Some(owner_id));
        let rules: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mock_rules WHERE token = ?")
            .bind(&token)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rules, 1);
        let traces: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM request_logs WHERE token = ?")
            .bind(&token)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(traces, 2);
        // second run is a no-op.
        assert!(seed_if_empty(&pool, 10).await.unwrap().is_none());
    }
}
