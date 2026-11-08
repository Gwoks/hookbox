//! Unit-test fixtures (compiled only under `cfg(test)`): a deterministic
//! `Config` that never reads the process environment, an in-memory migrated
//! pool, and the ONE place unit tests insert `owners` / `endpoints` rows
//! directly (everything else goes through the code under test).

use std::collections::HashMap;

use sqlx::SqlitePool;

use crate::config::Config;
use crate::db;
use crate::ids::{gen_owner_id, gen_owner_secret, hash_secret};
use crate::repo::endpoints::{self, NewEndpoint};
use crate::repo::owners;

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

/// An owner row seeded straight into the DB, plus its plaintext owner key.
pub(crate) struct SeededOwner {
    pub(crate) owner_id: String,
    pub(crate) key: String,
}

/// Insert a fresh anonymous owner with a random key (stored only as its hash).
pub(crate) async fn seed_owner(pool: &SqlitePool) -> SeededOwner {
    let key = gen_owner_secret(32);
    let owner_id = gen_owner_id();
    owners::insert(pool, &owner_id, &hash_secret(&key))
        .await
        .expect("insert owner");
    SeededOwner { owner_id, key }
}

/// Insert a live endpoint `token` owned by `owner_id` (schema defaults).
pub(crate) async fn seed_endpoint(pool: &SqlitePool, owner_id: &str, token: &str) {
    let new = NewEndpoint {
        token,
        owner_id,
        name: None,
        auto_crud: false,
    };
    endpoints::insert(pool, &new)
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
