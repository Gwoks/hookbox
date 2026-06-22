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

/// Outcome of a mutation closure run inside the transaction.
pub enum Mutation {
    /// New array + the item value to return to the caller.
    Ok(Vec<Value>, Value),
    /// The item id was not found.
    NotFound,
    /// A cap (item bytes) was exceeded.
    TooLarge,
}

#[derive(Debug)]
pub enum CrudError {
    Sqlx(sqlx::Error),
}

impl From<sqlx::Error> for CrudError {
    fn from(e: sqlx::Error) -> Self {
        CrudError::Sqlx(e)
    }
}

/// Read the current items array for a collection (live, non-expired).
pub async fn list_items(
    pool: &SqlitePool,
    token: &str,
    name: &str,
) -> Result<Vec<Value>, sqlx::Error> {
    let items: Option<String> = sqlx::query_scalar(
        "SELECT items_json FROM crud_collections WHERE token = ? AND name = ? AND expires_at > datetime('now')",
    )
    .bind(token)
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(parse_items(items))
}

fn parse_items(s: Option<String>) -> Vec<Value> {
    match s {
        Some(t) if !t.is_empty() => serde_json::from_str(&t).unwrap_or_default(),
        _ => Vec::new(),
    }
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
) -> Result<Mutation, CrudError> {
    let mut tx = pool.begin().await?;
    // BEGIN IMMEDIATE: take the write lock up front to serialize writers.
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *tx).await.ok();
    let current: Option<String> =
        sqlx::query_scalar("SELECT items_json FROM crud_collections WHERE token = ? AND name = ?")
            .bind(token)
            .bind(name)
            .fetch_optional(&mut *tx)
            .await?;
    let mut items = parse_items(current);
    if items.len() >= max_items {
        tx.rollback().await.ok();
        return Ok(Mutation::TooLarge);
    }
    let id = uuid::Uuid::new_v4().simple().to_string();
    obj.insert("id".into(), json!(id));
    let stored = Value::Object(obj);
    items.push(stored.clone());
    write_items(&mut tx, token, name, &items, ttl_seconds).await?;
    tx.commit().await?;
    Ok(Mutation::Ok(items, stored))
}

/// Replace an item by id (PUT). `id` is forced immutable.
pub async fn replace_item(
    pool: &SqlitePool,
    token: &str,
    name: &str,
    id: &str,
    mut obj: serde_json::Map<String, Value>,
    ttl_seconds: i64,
) -> Result<Mutation, CrudError> {
    obj.insert("id".into(), json!(id));
    cas_mutate(pool, token, name, ttl_seconds, |items| {
        match find_index(items, id) {
            Some(i) => {
                let v = Value::Object(obj.clone());
                items[i] = v.clone();
                Mutation::Ok(items.clone(), v)
            }
            None => Mutation::NotFound,
        }
    })
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
) -> Result<Mutation, CrudError> {
    cas_mutate(pool, token, name, ttl_seconds, |items| {
        match find_index(items, id) {
            Some(i) => {
                let mut merged = items[i].as_object().cloned().unwrap_or_default();
                for (k, v) in &patch {
                    merged.insert(k.clone(), v.clone());
                }
                merged.insert("id".into(), json!(id));
                let v = Value::Object(merged);
                if serde_json::to_string(&v).map(|s| s.len()).unwrap_or(0) > max_item_bytes {
                    return Mutation::TooLarge;
                }
                items[i] = v.clone();
                Mutation::Ok(items.clone(), v)
            }
            None => Mutation::NotFound,
        }
    })
    .await
}

/// Delete an item by id (DELETE).
pub async fn delete_item(
    pool: &SqlitePool,
    token: &str,
    name: &str,
    id: &str,
    ttl_seconds: i64,
) -> Result<Mutation, CrudError> {
    cas_mutate(pool, token, name, ttl_seconds, |items| {
        match find_index(items, id) {
            Some(i) => {
                items.remove(i);
                Mutation::Ok(items.clone(), Value::Null)
            }
            None => Mutation::NotFound,
        }
    })
    .await
}

/// Run a mutation closure under one `BEGIN IMMEDIATE` transaction (atomic
/// read-modify-write). The closure receives the parsed items and returns a
/// `Mutation`; on `Ok` the new array is persisted.
async fn cas_mutate<F>(
    pool: &SqlitePool,
    token: &str,
    name: &str,
    ttl_seconds: i64,
    f: F,
) -> Result<Mutation, CrudError>
where
    F: FnOnce(&mut Vec<Value>) -> Mutation,
{
    let mut tx = pool.begin().await?;
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *tx).await.ok();
    let current: Option<String> =
        sqlx::query_scalar("SELECT items_json FROM crud_collections WHERE token = ? AND name = ?")
            .bind(token)
            .bind(name)
            .fetch_optional(&mut *tx)
            .await?;
    let mut items = parse_items(current);
    match f(&mut items) {
        Mutation::Ok(new_items, ret) => {
            write_items(&mut tx, token, name, &new_items, ttl_seconds).await?;
            tx.commit().await?;
            Ok(Mutation::Ok(new_items, ret))
        }
        other => {
            tx.rollback().await.ok();
            Ok(other)
        }
    }
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
    use crate::db;

    async fn pool_ep() -> SqlitePool {
        let pool = db::pool(":memory:").await.unwrap();
        db::migrate(&pool).await.unwrap();
        sqlx::query("INSERT INTO owners (owner_id, email, secret_hash) VALUES ('o','e','h')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO endpoints (token, owner_id) VALUES ('tok','o')")
            .execute(&pool)
            .await
            .unwrap();
        pool
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
}
