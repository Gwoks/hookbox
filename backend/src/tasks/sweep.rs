//! Retention sweep — PORT of `app/utils/cleanup.py` intent + the reference
//! `tasks/sweep.rs` shape (AC-46..48, AC-58/59/61, AC-S13/S15).
//!
//! A `tokio::time::interval(RETENTION_SWEEP_SECONDS)` task that:
//!   (a) prunes `request_logs` beyond the newest `TRACE_CAP` per token AND past
//!       the 24h `TRACE_TTL_HOURS`,
//!   (b) deletes expired `endpoint_state` / `crud_collections` rows,
//!   (c) hard-deletes endpoint tombstones older than `GONE_TTL_HOURS`,
//!   (d) evicts idle rate-limit buckets so the in-memory map stays bounded.
//! The 100-cap is also held at write time (repo::requests::insert_trace) so it never drifts.

use std::sync::Arc;
use std::time::Duration;

use sqlx::SqlitePool;

use crate::config::Config;
use crate::limiter::Limiter;
use crate::repo::retention;

/// Run one sweep pass. Returned tuple is `(traces_pruned, state_reaped,
/// crud_reaped, tombstones_reaped)` for observability/tests.
pub async fn sweep_once(
    pool: &SqlitePool,
    cfg: &Config,
) -> Result<(u64, u64, u64, u64), sqlx::Error> {
    // (a) TTL prune of traces past TRACE_TTL_HOURS, then the per-token cap.
    let traces_ttl = retention::prune_traces_older_than(pool, cfg.trace_ttl_hours).await?;
    let cap_pruned = retention::cap_traces_per_token(pool, cfg.trace_cap).await?;
    // (b) expired state + crud.
    let state_reaped = retention::reap_expired_state(pool).await?;
    let crud_reaped = retention::reap_expired_crud(pool).await?;
    // (c) tombstones older than GONE_TTL_HOURS -> hard delete (token degrades to 404).
    let tombstones = retention::reap_tombstones_older_than(pool, cfg.gone_ttl_hours).await?;

    Ok((
        traces_ttl + cap_pruned,
        state_reaped,
        crud_reaped,
        tombstones,
    ))
}

/// Spawn the periodic sweep loop. Returns the join handle (kept alive by main).
pub fn spawn(
    pool: SqlitePool,
    cfg: Arc<Config>,
    limiter: Arc<Limiter>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick =
            tokio::time::interval(Duration::from_secs(cfg.retention_sweep_seconds.max(1)));
        // Skip the immediate first tick fire.
        tick.tick().await;
        loop {
            tick.tick().await;
            if let Err(e) = sweep_once(&pool, &cfg).await {
                tracing::warn!("retention sweep error (continuing): {e}");
            }
            // Evict rate buckets idle longer than two sweep intervals.
            limiter.evict_idle(cfg.retention_sweep_seconds * 2);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{pool_with_endpoint, test_config};

    #[tokio::test]
    async fn reaps_expired_state_crud_and_tombstones() {
        let (p, owner) = pool_with_endpoint("tok").await;
        // already-expired state + crud
        sqlx::query("INSERT INTO endpoint_state (token,key,value,expires_at) VALUES ('tok','k','v',datetime('now','-1 hour'))")
            .execute(&p).await.unwrap();
        sqlx::query("INSERT INTO crud_collections (token,name,expires_at) VALUES ('tok','c',datetime('now','-1 hour'))")
            .execute(&p).await.unwrap();
        // old tombstone
        sqlx::query("INSERT INTO endpoints (token,owner_id,gone_at) VALUES ('gone',?,datetime('now','-200 hours'))")
            .bind(&owner.owner_id)
            .execute(&p).await.unwrap();

        let (_t, state_reaped, crud_reaped, tombstones) =
            sweep_once(&p, &test_config()).await.unwrap();
        assert_eq!(state_reaped, 1);
        assert_eq!(crud_reaped, 1);
        assert_eq!(tombstones, 1);
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM endpoints WHERE token='gone'")
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(n, 0);
    }

    #[tokio::test]
    async fn caps_traces_per_token() {
        let (p, _owner) = pool_with_endpoint("tok").await;
        let c = Config {
            trace_cap: 5,
            ..test_config()
        };
        for i in 0..12 {
            sqlx::query("INSERT INTO request_logs (token,method,path,status_code,served_by) VALUES ('tok','GET',?,200,'default')")
                .bind(format!("/{i}"))
                .execute(&p).await.unwrap();
        }
        sweep_once(&p, &c).await.unwrap();
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM request_logs WHERE token='tok'")
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(n, 5);
    }
}
