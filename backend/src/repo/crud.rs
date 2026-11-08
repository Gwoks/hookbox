//! Auto-CRUD store over the `crud_collections` table (replaces Redis
//! `crud:<token>:<coll>`). §5.5 / AC-24..27, AC-S17.
//!
//! Each collection is a JSON array of objects, each with a server-assigned uuid
//! `id`. Mutations run as an atomic read-modify-write inside one
//! `BEGIN IMMEDIATE` transaction: the write lock + `busy_timeout` serialize
//! concurrent writers to the same collection (replacing the Redis WATCH/MULTI
//! CAS), so no update is lost (AC-26) and two POSTs never collide. Item/byte caps
//! are enforced in-transaction. The collection row is upserted with a fresh TTL.

use serde_json::{json, Value};
use sqlx::SqlitePool;

use super::begin_immediate;
use crate::util::json::parse_json_or;

/// Outcome of a mutation closure run inside the transaction.
pub enum Mutation {
    /// New array + the item value to return to the caller.
    Ok(Vec<Value>, Value),
    /// The item id was not found.
    NotFound,
    /// A cap (item bytes) was exceeded.
    TooLarge,
}

/// The stored items JSON of a live (non-expired) collection, verbatim.
pub(crate) async fn read_items_json(
    pool: &SqlitePool,
    token: &str,
    name: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT items_json FROM crud_collections WHERE token = ? AND name = ? AND expires_at > datetime('now')",
    )
    .bind(token)
    .bind(name)
    .fetch_optional(pool)
    .await
}

/// Read the current items array for a collection (live, non-expired).
pub async fn list_items(
    pool: &SqlitePool,
    token: &str,
    name: &str,
) -> Result<Vec<Value>, sqlx::Error> {
    let items = read_items_json(pool, token, name).await?;
    Ok(parse_json_or(items.as_deref(), Vec::new()))
}

/// Drop a collection outright (management #18).
pub(crate) async fn clear(pool: &SqlitePool, token: &str, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM crud_collections WHERE token = ? AND name = ?")
        .bind(token)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}

/// Persist a new items array for a collection inside the calling transaction.
async fn write_items(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    token: &str,
    name: &str,
    items: &[Value],
    ttl_seconds: i64,
) -> Result<(), sqlx::Error> {
    let items_json = serde_json::to_string(items).unwrap_or_else(|_| "[]".into());
    let expires = format!("{ttl_seconds:+} seconds");
    sqlx::query(
        "INSERT INTO crud_collections (token, name, items_json, updated_at, expires_at)
         VALUES (?, ?, ?, datetime('now'), datetime('now', ?))
         ON CONFLICT(token, name) DO UPDATE SET
             items_json = excluded.items_json,
             updated_at = datetime('now'),
             expires_at = excluded.expires_at",
    )
    .bind(token)
    .bind(name)
    .bind(&items_json)
    .bind(&expires)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Append a new item (server-assigned uuid `id`). Returns the stored item, or
/// `Mutation::TooLarge` when `CRUD_MAX_ITEMS` would be exceeded.
pub async fn append_item(
    pool: &SqlitePool,
    token: &str,
    name: &str,
    mut obj: serde_json::Map<String, Value>,
    max_items: usize,
    ttl_seconds: i64,
) -> Result<Mutation, sqlx::Error> {
    cas_mutate(pool, token, name, ttl_seconds, |mut items| {
        if items.len() >= max_items {
            return Mutation::TooLarge;
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        obj.insert("id".into(), json!(id));
        let stored = Value::Object(obj);
        items.push(stored.clone());
        Mutation::Ok(items, stored)
    })
    .await
}

/// Replace an item by id (PUT). `id` is forced immutable.
pub async fn replace_item(
    pool: &SqlitePool,
    token: &str,
    name: &str,
    id: &str,
    mut obj: serde_json::Map<String, Value>,
    ttl_seconds: i64,
) -> Result<Mutation, sqlx::Error> {
    obj.insert("id".into(), json!(id));
    cas_mutate(
        pool,
        token,
        name,
        ttl_seconds,
        |mut items| match find_index(&items, id) {
            Some(i) => {
                let v = Value::Object(obj);
                items[i] = v.clone();
                Mutation::Ok(items, v)
            }
            None => Mutation::NotFound,
        },
    )
    .await
}

/// Shallow-merge an item by id (PATCH). `id` immutable; enforces item byte cap.
pub async fn merge_item(
    pool: &SqlitePool,
    token: &str,
    name: &str,
    id: &str,
    patch: serde_json::Map<String, Value>,
    max_item_bytes: usize,
    ttl_seconds: i64,
) -> Result<Mutation, sqlx::Error> {
    cas_mutate(
        pool,
        token,
        name,
        ttl_seconds,
        |mut items| match find_index(&items, id) {
            Some(i) => {
                let mut merged = items[i].as_object().cloned().unwrap_or_default();
                merged.extend(patch);
                merged.insert("id".into(), json!(id));
                let v = Value::Object(merged);
                if serde_json::to_string(&v).map(|s| s.len()).unwrap_or(0) > max_item_bytes {
                    return Mutation::TooLarge;
                }
                items[i] = v.clone();
                Mutation::Ok(items, v)
            }
            None => Mutation::NotFound,
        },
    )
    .await
}

/// Delete an item by id (DELETE).
pub async fn delete_item(
    pool: &SqlitePool,
    token: &str,
    name: &str,
    id: &str,
    ttl_seconds: i64,
) -> Result<Mutation, sqlx::Error> {
    cas_mutate(
        pool,
        token,
        name,
        ttl_seconds,
        |mut items| match find_index(&items, id) {
            Some(i) => {
                items.remove(i);
                Mutation::Ok(items, Value::Null)
            }
            None => Mutation::NotFound,
        },
    )
    .await
}

/// Run a mutation closure as one atomic read-modify-write under
/// `repo::begin_immediate` (the write lock is held BEFORE the read, so a
/// concurrent writer can never invalidate the snapshot this mutates). The
/// closure receives the parsed items and returns a `Mutation`; on `Ok` the
/// new array is persisted and committed, otherwise the transaction is
/// dropped (rolled back) untouched.
async fn cas_mutate<F>(
    pool: &SqlitePool,
    token: &str,
    name: &str,
    ttl_seconds: i64,
    f: F,
) -> Result<Mutation, sqlx::Error>
where
    F: FnOnce(Vec<Value>) -> Mutation,
{
    let mut tx = begin_immediate(pool).await?;
    let current: Option<String> =
        sqlx::query_scalar("SELECT items_json FROM crud_collections WHERE token = ? AND name = ?")
            .bind(token)
            .bind(name)
            .fetch_optional(&mut *tx)
            .await?;
    let outcome = f(parse_json_or(current.as_deref(), Vec::new()));
    if let Mutation::Ok(new_items, _) = &outcome {
        write_items(&mut tx, token, name, new_items, ttl_seconds).await?;
        tx.commit().await?;
    }
    Ok(outcome)
}

fn find_index(items: &[Value], id: &str) -> Option<usize> {
    items.iter().position(|it| {
        it.as_object()
            .and_then(|o| o.get("id"))
            .and_then(|v| v.as_str())
            .map(|s| s == id)
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
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
}
