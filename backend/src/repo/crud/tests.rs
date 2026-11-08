//! Auto-CRUD store tests: lifecycle, caps, and write-lock behavior (B8).

use super::*;
use crate::testing::pool_with_endpoint;

async fn pool_ep() -> SqlitePool {
    pool_with_endpoint("tok").await.0
}

fn obj(pairs: &[(&str, Value)]) -> serde_json::Map<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

#[tokio::test]
async fn full_lifecycle() {
    let pool = pool_ep().await;
    let m = append_item(
        &pool,
        "tok",
        "books",
        obj(&[("title", json!("Dune"))]),
        1000,
        86400,
    )
    .await
    .unwrap();
    let id = match m {
        Mutation::Ok(_, v) => v["id"].as_str().unwrap().to_string(),
        _ => panic!(),
    };
    assert_eq!(id.len(), 32); // uuid simple
    let items = list_items(&pool, "tok", "books").await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["title"], json!("Dune"));

    // PUT replace
    let m = replace_item(
        &pool,
        "tok",
        "books",
        &id,
        obj(&[("title", json!("Foundation"))]),
        86400,
    )
    .await
    .unwrap();
    assert!(matches!(m, Mutation::Ok(..)));
    assert_eq!(
        list_items(&pool, "tok", "books").await.unwrap()[0]["title"],
        json!("Foundation")
    );

    // PATCH merge keeps id
    merge_item(
        &pool,
        "tok",
        "books",
        &id,
        obj(&[("year", json!(1951))]),
        64000,
        86400,
    )
    .await
    .unwrap();
    let after = list_items(&pool, "tok", "books").await.unwrap();
    assert_eq!(after[0]["year"], json!(1951));
    assert_eq!(after[0]["id"], json!(id));

    // DELETE
    assert!(matches!(
        delete_item(&pool, "tok", "books", &id, 86400)
            .await
            .unwrap(),
        Mutation::Ok(..)
    ));
    assert!(list_items(&pool, "tok", "books").await.unwrap().is_empty());
    // delete missing -> NotFound
    assert!(matches!(
        delete_item(&pool, "tok", "books", "nope", 86400)
            .await
            .unwrap(),
        Mutation::NotFound
    ));
}

#[tokio::test]
async fn max_items_cap() {
    let pool = pool_ep().await;
    append_item(&pool, "tok", "c", obj(&[]), 1, 86400)
        .await
        .unwrap();
    // cap=1 reached -> TooLarge
    assert!(matches!(
        append_item(&pool, "tok", "c", obj(&[]), 1, 86400)
            .await
            .unwrap(),
        Mutation::TooLarge
    ));
}

#[tokio::test]
async fn concurrent_posts_no_collision() {
    let pool = pool_ep().await;
    let mut handles = Vec::new();
    for i in 0..20 {
        let p = pool.clone();
        handles.push(tokio::spawn(async move {
            append_item(&p, "tok", "items", obj(&[("n", json!(i))]), 1000, 86400)
                .await
                .unwrap();
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
    let items = list_items(&pool, "tok", "items").await.unwrap();
    assert_eq!(items.len(), 20, "no lost update under concurrent writers");
    let mut ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 20, "uuid ids never collide");
}

/// A migrated, file-backed WAL pool with one endpoint `tok`. The
/// in-memory pool uses shared-cache table locks, which hide the DEFERRED
/// read-then-write upgrade failure (`SQLITE_BUSY_SNAPSHOT`) that real WAL
/// databases hit under concurrent writers.
async fn wal_file_pool() -> (tempfile::TempDir, SqlitePool) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("crud.db");
    let pool = crate::db::pool(path.to_str().expect("utf-8 path"))
        .await
        .expect("open WAL pool");
    crate::db::migrate(&pool).await.expect("migrate");
    let owner = crate::testing::seed_owner(&pool).await;
    crate::testing::seed_endpoint(&pool, &owner.owner_id, "tok").await;
    (dir, pool)
}

/// B8: `begin_immediate` really holds the write lock from BEGIN until
/// commit — a second writer (short busy timeout) is refused meanwhile.
#[tokio::test]
async fn begin_immediate_holds_the_write_lock_until_commit() {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    let (dir, _pool) = wal_file_pool().await;
    let opts = SqliteConnectOptions::new()
        .filename(dir.path().join("crud.db"))
        .busy_timeout(std::time::Duration::from_millis(50));
    let fast = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .unwrap();
    let wipe = "DELETE FROM crud_collections";
    let tx = begin_immediate(&fast).await.unwrap();
    let blocked = sqlx::query(wipe).execute(&fast).await;
    assert!(
        blocked.is_err(),
        "a second writer must wait on the held lock"
    );
    tx.commit().await.unwrap();
    sqlx::query(wipe).execute(&fast).await.unwrap();
}

/// B8: a read-modify-write must take the write lock BEFORE it reads.
/// Another writer holds the lock and commits a change to the same
/// collection while `append_item` is in flight: under a DEFERRED
/// transaction the append reads the stale snapshot and then either fails
/// its lock upgrade (`SQLITE_BUSY_SNAPSHOT`) or overwrites the other
/// write; under `BEGIN IMMEDIATE` it waits on `busy_timeout`, then reads
/// the committed row and appends to it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn append_waits_for_a_concurrent_writer_instead_of_losing_its_update() {
    let (_dir, pool) = wal_file_pool().await;
    let mut other = pool.acquire().await.unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *other)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO crud_collections (token, name, items_json, expires_at)
         VALUES ('tok', 'items', '[{\"id\":\"first\"}]', datetime('now', '+1 day'))",
    )
    .execute(&mut *other)
    .await
    .unwrap();

    let p = pool.clone();
    let append = tokio::spawn(async move {
        append_item(&p, "tok", "items", obj(&[("n", json!(2))]), 1000, 86400).await
    });
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    sqlx::query("COMMIT").execute(&mut *other).await.unwrap();

    let outcome = append.await.expect("append task");
    assert!(outcome.is_ok(), "append failed: {:?}", outcome.err());
    let items = list_items(&pool, "tok", "items").await.unwrap();
    assert_eq!(items.len(), 2, "both writes survive: {items:?}");
    assert_eq!(items[0]["id"], json!("first"));
}
