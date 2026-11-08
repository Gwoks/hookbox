//! First-run demo-data seeding — PORT of the intent of the Python seed (AC-53/54,
//! AC-J12). Plants an anonymous demo owner + a primary endpoint with >=1 rule +
//! sample traces so a fresh dashboard lands populated. Idempotent: a no-op when
//! any endpoint already exists.

use sqlx::SqlitePool;

use crate::ids::{gen_owner_id, gen_owner_secret, gen_token, hash_secret};
use crate::models::{MatchCriteria, MockRuleCreate, ResponseSpec};
use crate::repo::endpoints::{self, NewEndpoint};
use crate::repo::requests::{insert_trace, TraceRecord};
use crate::repo::{owners, rules};

/// Entropy of the demo owner's key (the `OWNER_SECRET_BYTES` floor).
const DEMO_KEY_BYTES: usize = 32;

/// Write-time trace cap for the seeded traces (well above the two planted).
const DEMO_TRACE_CAP: i64 = 100;

/// The demo rule: GET /hello -> 200 {"hello":"world"} with a template tag.
fn demo_rule() -> MockRuleCreate {
    MockRuleCreate {
        name: Some("Hello rule".to_string()),
        priority: 100,
        enabled: true,
        r#match: MatchCriteria {
            method: "GET".to_string(),
            path: "/hello".to_string(),
            ..MatchCriteria::default()
        },
        response: ResponseSpec {
            body_template: "{\"hello\":\"world\",\"at\":\"{{now 'iso'}}\"}".to_string(),
            ..ResponseSpec::default()
        },
        state_writes: Vec::new(),
        latency_ms: None,
        rate_limit_per_min: None,
        chaos_mode: None,
        webhook_action: None,
    }
}

/// A sample `GET <path>` trace served by `served_by` (`n` scales timings).
fn demo_trace(token: &str, path: &str, served_by: &str, n: i64) -> TraceRecord {
    TraceRecord {
        token: token.to_string(),
        method: "GET".to_string(),
        path: path.to_string(),
        status_code: 200,
        served_by: served_by.to_string(),
        matched_rule_id: None,
        duration_ms: n * 2,
        overhead_ms: n,
        request_headers: "{}".to_string(),
        query_params: "{}".to_string(),
        request_body: None,
        response_headers: "{}".to_string(),
        response_body: None,
        trace_json: "[]".to_string(),
        state_snapshot: "{}".to_string(),
    }
}

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
    if endpoints::any_exist(pool).await? {
        return Ok(None);
    }

    let owner_id = gen_owner_id();
    let key = gen_owner_secret(DEMO_KEY_BYTES);
    owners::insert(pool, &owner_id, &hash_secret(&key)).await?;

    let token = gen_token(id_len);
    let endpoint = NewEndpoint {
        token: &token,
        owner_id: &owner_id,
        name: Some("Demo API"),
        auto_crud: true,
    };
    endpoints::insert(pool, &endpoint).await?;
    rules::insert(pool, &token, &demo_rule()).await?;

    // A couple of sample traces so the first-run feed is populated.
    for (i, (path, served_by)) in [("/hello", "rule"), ("/widgets", "crud")]
        .into_iter()
        .enumerate()
    {
        let n = i as i64 + 1;
        insert_trace(
            pool,
            &demo_trace(&token, path, served_by, n),
            DEMO_TRACE_CAP,
        )
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
