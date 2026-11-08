//! Unit-test fixtures (compiled only under `cfg(test)`): a deterministic
//! `Config` that never reads the process environment, an in-memory migrated
//! pool, and the ONE place unit tests insert `owners` / `endpoints` rows
//! directly (everything else goes through the code under test).

use std::collections::HashMap;

use sqlx::SqlitePool;

use crate::config::Config;
use crate::db;
use crate::ids::{gen_owner_secret, hash_secret};

/// A `Config` loaded from exactly `pairs`; every other knob takes its
/// documented default. Never touches `std::env`, so tests stay parallel-safe.
pub(crate) fn config_from(pairs: &[(&str, &str)]) -> Config {
    let vars: HashMap<&str, &str> = pairs.iter().copied().collect();
    Config::from_source(|key| vars.get(key).map(|v| v.to_string()))
}

/// The standard unit-test config: wildcard mock domain `mock.local`, app host
/// `app.local`, everything else default.
pub(crate) fn test_config() -> Config {
    config_from(&[("MOCK_DOMAIN", "mock.local"), ("APP_HOST", "app.local")])
}

/// A fresh in-memory SQLite pool with every migration applied.
pub(crate) async fn memory_pool() -> SqlitePool {
    let pool = db::pool(":memory:")
        .await
        .expect("open in-memory SQLite pool");
    db::migrate(&pool).await.expect("apply migrations");
    pool
}

/// An owner row seeded straight into the DB, plus its plaintext secret.
pub(crate) struct SeededOwner {
    pub(crate) owner_id: String,
    pub(crate) secret: String,
}

/// Insert a fresh owner with a random secret (stored only as its hash).
pub(crate) async fn seed_owner(pool: &SqlitePool) -> SeededOwner {
    let secret = gen_owner_secret(32);
    let secret_hash = hash_secret(&secret);
    let owner_id = secret_hash[..16].to_string();
    sqlx::query("INSERT INTO owners (owner_id, email, secret_hash) VALUES (?, ?, ?)")
        .bind(&owner_id)
        .bind(format!("{owner_id}@seed.test"))
        .bind(&secret_hash)
        .execute(pool)
        .await
        .expect("insert owner");
    SeededOwner { owner_id, secret }
}

/// Insert a live endpoint `token` owned by `owner_id` (schema defaults).
pub(crate) async fn seed_endpoint(pool: &SqlitePool, owner_id: &str, token: &str) {
    sqlx::query("INSERT INTO endpoints (token, owner_id) VALUES (?, ?)")
        .bind(token)
        .bind(owner_id)
        .execute(pool)
        .await
        .expect("insert endpoint");
}

/// A migrated pool holding one seeded owner and one live endpoint `token`.
pub(crate) async fn pool_with_endpoint(token: &str) -> (SqlitePool, SeededOwner) {
    let pool = memory_pool().await;
    let owner = seed_owner(&pool).await;
    seed_endpoint(&pool, &owner.owner_id, token).await;
    (pool, owner)
}
